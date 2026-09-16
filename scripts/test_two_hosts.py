"""Run native core-to-core tests. SSH controls the remote process; TLS uses --address.

Build link_probe with --features diagnostics on both hosts first. The Linux host
must have its own isolated display (never an SSH-forwarded X11 display).
"""
import argparse
import hashlib
import json
import os
import queue
import shlex
import subprocess
import sys
import threading
import time


class Probe:
    def __init__(self, command):
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        text=True, encoding="utf-8", bufsize=1)
        self.output = queue.Queue()
        self.serial = 0
        self.events = []

        def read():
            for line in self.process.stdout:
                self.output.put(json.loads(line))
            self.output.put({"eof": True})

        threading.Thread(target=read, daemon=True).start()
        self.identity = self.output.get(timeout=15)
        assert self.identity.get("ready"), self.identity

    def call(self, command, **args):
        self.serial += 1
        self.process.stdin.write(json.dumps({"id": self.serial, "command": command, **args}) + "\n")
        self.process.stdin.flush()
        response = self.output.get(timeout=15)
        assert response.get("id") == self.serial and response.get("ok"), response
        return response["result"]

    def collect(self):
        events = self.call("events")
        self.events.extend(events)
        return events

    def close(self):
        try:
            if self.process.poll() is None:
                self.call("quit")
                self.process.wait(timeout=8)
        finally:
            if self.process.poll() is None:
                self.process.terminate()
                self.process.wait(timeout=8)
            self.process.stdin.close()
            self.process.stdout.close()


def digest(text):
    data = text.encode()
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def eventually(check, reason, timeout=12):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(0.05)
    raise AssertionError(reason)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", required=True)
    parser.add_argument("--remote-dir", required=True)
    parser.add_argument("--address", required=True, help="actual listener IP:port, not an SSH tunnel")
    parser.add_argument("--listener", choices=["local", "remote"], default="remote")
    parser.add_argument("--backend", choices=["x11", "wayland"], required=True)
    parser.add_argument("--display", default=":97")
    parser.add_argument("--wayland-display", default="wayland-1")
    args = parser.parse_args()
    env = ["env", "NOOBOARD_LINUX_BACKEND=" + args.backend,
           "DISPLAY=" + args.display,
           "XDG_RUNTIME_DIR=" + args.remote_dir + "/runtime",
           "WAYLAND_DISPLAY=" + args.wayland_display]
    remote_command = shlex.join(env + [args.remote_dir + "/source/target/debug/examples/link_probe"])
    peers = []
    try:
        local = Probe([os.path.abspath("target/debug/examples/link_probe")])
        peers.append(local)
        remote = Probe(["ssh", "-x", "-T", args.host, remote_command])
        peers.append(remote)

        listener = remote if args.listener == "remote" else local
        listener.call("listen", address=args.address)

        def pair_one(owner, other, address):
            owner.call("trust_peer", certificate=other.identity["certificate"],
                       fingerprint=other.identity["fingerprint"],
                       device_name=other.identity["device_name"], address=address)
            owner.call("targets", targets=[other.identity["noob_id"]])

        def pair_remote():
            pair_one(remote, local, args.address if args.listener == "local" else None)

        pair_remote()
        pair_one(local, remote, args.address if args.listener == "remote" else None)

        def state(owner, other):
            return next((p for p in owner.call("status")["peers"]
                         if p["noob_id"] == other.identity["noob_id"]), {})

        def ready():
            return all(state(owner, other).get("online") and state(owner, other).get("accepting")
                       for owner, other in [(local, remote), (remote, local)])

        eventually(ready, "two-way TLS handshake/readiness")
        transfers = []

        def send(sender, receiver, text):
            sender.call("copy", text=text)
            message_id = sender.call("send")
            eventually(lambda: receiver.call("read") == digest(text), "native clipboard application")
            eventually(lambda: any(t["id"] == message_id and all(d["state"] == "Applied" for d in t["targets"])
                                   for t in sender.call("status")["transfers"]), "remote Applied acknowledgement")
            transfers.append(digest(text))

        for index, text in enumerate([" 中文 🦀\r\nline\n  ", "长文本🦀" * 60000, "x" * (1 << 20)]):
            send(local, remote, text)
            send(remote, local, ("回" + str(index) + "\n") if index == 2 else text[::-1])

        for peer in peers:
            for device in peer.call("status")["peers"]:
                device["settings"]["auto_send"] = True
                peer.call("configure_peer", noob_id=device["noob_id"], settings=device["settings"])
            settings = peer.call("status")["settings"]
            settings["mode"] = "Automatic"
            peer.call("settings", settings=settings)
            peer.collect()
        before = [len(p.events) for p in peers]
        local.call("copy", text="自动同步 Mac → Ubuntu 🦀")
        eventually(lambda: remote.call("read") == digest("自动同步 Mac → Ubuntu 🦀"), "automatic Mac to Ubuntu")
        remote.call("copy", text="自动同步 Ubuntu → Mac 🦀")
        eventually(lambda: local.call("read") == digest("自动同步 Ubuntu → Mac 🦀"), "automatic Ubuntu to Mac")
        time.sleep(0.4)
        for p, offset in zip(peers, before):
            p.collect()
            sent_ids = {json.dumps(e["Transfer"]["id"], sort_keys=True) for e in p.events[offset:] if "Transfer" in e}
            assert len(sent_ids) == 1, "clipboard echo loop"

        settings = remote.call("status")["settings"]
        settings["paused"] = True
        remote.call("settings", settings=settings)
        eventually(lambda: not state(local, remote).get("accepting"), "pause advertisement")
        previous = remote.call("read")
        local.call("copy", text="paused content must not replay")
        time.sleep(0.3)
        assert remote.call("read") == previous
        settings["paused"] = False
        remote.call("settings", settings=settings)
        eventually(ready, "resume")
        time.sleep(0.3)
        assert remote.call("read") == previous, "paused text replayed"

        remote.call("unpair", noob_id=local.identity["noob_id"])
        eventually(lambda: not state(local, remote).get("online"), "disconnect")
        local.call("copy", text="offline content must not replay")
        time.sleep(0.3)
        pair_remote()
        eventually(ready, "reconnect")
        time.sleep(0.3)
        assert remote.call("read") == previous, "offline text replayed"
        local.call("send")
        eventually(lambda: remote.call("read") == digest("offline content must not replay"), "explicit send after reconnect")
        histories = [p.call("history") for p in peers]
        for entries in histories:
            assert any(e["text"] == digest("x" * (1 << 20)) for e in entries), "history did not preserve full text"
            assert len({e["text"]["sha256"] for e in entries}) == len(entries), "history duplicated"
        print(json.dumps({"passed": True, "backend": args.backend, "address": args.address,
                          "listener": args.listener, "transport": "direct mutual TLS 1.3; SSH control only",
                          "manual_transfers": transfers, "automatic_directions": 2,
                          "echo_suppressed": True, "pause_and_reconnect_no_replay": True,
                          "history_entries": [len(h) for h in histories]}, ensure_ascii=False))
    except BaseException:
        for index, peer in enumerate(peers):
            if peer.process.poll() is None:
                print(json.dumps({"peer": index, "status": peer.call("status"), "events": peer.collect()}), file=sys.stderr)
        raise
    finally:
        for peer in reversed(peers):
            peer.close()


if __name__ == "__main__":
    main()
