use super::*;
impl Runtime {
    pub(super) fn start(
        &mut self,
        kind: StartKind,
        cancellation: CancellationToken,
        output: bool,
        reply: Reply<(RunId, Option<TerminalOutputSubscription>)>,
    ) {
        let prepared = || {
            self.admit()?;
            if cancellation.is_cancelled() {
                return Err(cancelled());
            }
            let mut plan = StartPlan {
                root: kind,
                catalog: self.catalog.clone(),
            };
            let references = plan.references()?;
            // Retain only the transitive frozen input; trust is queried live at handshake time.
            plan.catalog
                .ssh_targets
                .retain(|id, _| references.contains(&TerminalConfigReference::SshTarget(*id)));
            plan.catalog
                .commands
                .retain(|id, _| references.contains(&TerminalConfigReference::Command(*id)));
            plan.catalog
                .terminals
                .retain(|id, _| references.contains(&TerminalConfigReference::Terminal(*id)));
            plan.catalog
                .tunnels
                .retain(|id, _| references.contains(&TerminalConfigReference::Tunnel(*id)));
            plan.catalog.trusted_hosts.clear();
            Ok((plan, self.references.reserve(None, &references)))
        };
        let (plan, reservation) = match prepared() {
            Ok(value) => value,
            Err(error) => {
                let _ = reply.send(Err(error));
                return;
            }
        };
        let engine = self.engine.clone();
        self.tasks.spawn(async move {
            let _reservation = reservation;
            let result = engine.start(plan, cancellation, output).await;
            if let Err(Ok((run, _))) = reply.send(result) {
                // A lost root start reply must not leave an unreachable terminal or tunnel.
                // Shared commands may already have other dependents and remain managed.
                match run {
                    RunId::Terminal(id) => {
                        let _ = engine.stop_terminal(id).await;
                    }
                    RunId::Tunnel(id) => {
                        let _ = engine.stop_tunnel(id).await;
                    }
                    _ => {}
                }
            }
        });
    }
}
