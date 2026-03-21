use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;

use crate::{ui::workspace::WorkspaceView, workspace::view_state::SettingsStorageViewState};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::workspace) enum StorageDurationUnit {
    Days,
    Weeks,
    Months,
}

impl StorageDurationUnit {
    pub(in crate::ui::workspace) const ALL: [Self; 3] =
        [Self::Days, Self::Weeks, Self::Months];

    pub(in crate::ui::workspace) fn label(self) -> &'static str {
        match self {
            Self::Days => "Days",
            Self::Weeks => "Weeks",
            Self::Months => "Months",
        }
    }

    fn from_days(days: u32) -> Self {
        if days % 30 == 0 {
            Self::Months
        } else if days % 7 == 0 {
            Self::Weeks
        } else {
            Self::Days
        }
    }

    fn to_days(self, amount: f64) -> f64 {
        match self {
            Self::Days => amount,
            Self::Weeks => amount * 7.0,
            Self::Months => amount * 30.0,
        }
    }

    fn value_from_days(self, days: u32) -> f64 {
        match self {
            Self::Days => f64::from(days),
            Self::Weeks => f64::from(days) / 7.0,
            Self::Months => f64::from(days) / 30.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::workspace) enum StorageBytesUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
}

impl StorageBytesUnit {
    pub(in crate::ui::workspace) const ALL: [Self; 4] = [
        Self::Bytes,
        Self::Kilobytes,
        Self::Megabytes,
        Self::Gigabytes,
    ];

    pub(in crate::ui::workspace) fn label(self) -> &'static str {
        match self {
            Self::Bytes => "Bytes",
            Self::Kilobytes => "KB",
            Self::Megabytes => "MB",
            Self::Gigabytes => "GB",
        }
    }

    fn from_bytes(bytes: usize) -> Self {
        const KIB: usize = 1024;
        const MIB: usize = 1024 * 1024;
        const GIB: usize = 1024 * 1024 * 1024;

        if bytes >= GIB && bytes % GIB == 0 {
            Self::Gigabytes
        } else if bytes >= MIB && bytes % MIB == 0 {
            Self::Megabytes
        } else if bytes >= KIB && bytes % KIB == 0 {
            Self::Kilobytes
        } else {
            Self::Bytes
        }
    }

    fn factor(self) -> f64 {
        match self {
            Self::Bytes => 1.0,
            Self::Kilobytes => 1024.0,
            Self::Megabytes => 1024.0 * 1024.0,
            Self::Gigabytes => 1024.0 * 1024.0 * 1024.0,
        }
    }

    fn value_from_bytes(self, bytes: usize) -> f64 {
        bytes as f64 / self.factor()
    }
}

#[derive(Clone)]
struct StorageSyncedValues {
    history_window_days: u32,
    dedup_window_days: u32,
    max_text_bytes: usize,
}

pub(super) struct StorageSettingsState {
    history_window_input: Entity<InputState>,
    dedup_window_input: Entity<InputState>,
    max_text_bytes_input: Entity<InputState>,
    history_window_unit: StorageDurationUnit,
    dedup_window_unit: StorageDurationUnit,
    max_text_bytes_unit: StorageBytesUnit,
    synced: Option<StorageSyncedValues>,
}

impl StorageSettingsState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<WorkspaceView>) -> Self {
        Self {
            history_window_input: cx.new(|cx| InputState::new(window, cx).placeholder("7")),
            dedup_window_input: cx.new(|cx| InputState::new(window, cx).placeholder("14")),
            max_text_bytes_input: cx.new(|cx| InputState::new(window, cx).placeholder("4096")),
            history_window_unit: StorageDurationUnit::Days,
            dedup_window_unit: StorageDurationUnit::Days,
            max_text_bytes_unit: StorageBytesUnit::Bytes,
            synced: None,
        }
    }

    pub(super) fn input_entities(&self) -> Vec<Entity<InputState>> {
        vec![
            self.history_window_input.clone(),
            self.dedup_window_input.clone(),
            self.max_text_bytes_input.clone(),
        ]
    }

    pub(super) fn sync_from_workspace(
        &mut self,
        page: &SettingsStorageViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let next = StorageSyncedValues {
            history_window_days: page.history_window_days,
            dedup_window_days: page.dedup_window_days,
            max_text_bytes: page.max_text_bytes,
        };

        if let Some(previous) = self.synced.clone() {
            self.sync_duration_input_if_clean(
                &self.history_window_input,
                previous.history_window_days,
                next.history_window_days,
                self.history_window_unit,
                window,
                cx,
            );
            self.sync_duration_input_if_clean(
                &self.dedup_window_input,
                previous.dedup_window_days,
                next.dedup_window_days,
                self.dedup_window_unit,
                window,
                cx,
            );
            self.sync_bytes_input_if_clean(
                &self.max_text_bytes_input,
                previous.max_text_bytes,
                next.max_text_bytes,
                self.max_text_bytes_unit,
                window,
                cx,
            );
        } else {
            self.reset_from_workspace(page, window, cx);
            return;
        }

        self.synced = Some(next);
    }

    pub(super) fn reset_from_workspace(
        &mut self,
        page: &SettingsStorageViewState,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.history_window_unit = StorageDurationUnit::from_days(page.history_window_days);
        self.dedup_window_unit = StorageDurationUnit::from_days(page.dedup_window_days);
        self.max_text_bytes_unit = StorageBytesUnit::from_bytes(page.max_text_bytes);

        self.set_input_value(
            &self.history_window_input,
            format_duration_value(page.history_window_days, self.history_window_unit),
            window,
            cx,
        );
        self.set_input_value(
            &self.dedup_window_input,
            format_duration_value(page.dedup_window_days, self.dedup_window_unit),
            window,
            cx,
        );
        self.set_input_value(
            &self.max_text_bytes_input,
            format_bytes_value(page.max_text_bytes, self.max_text_bytes_unit),
            window,
            cx,
        );

        self.synced = Some(StorageSyncedValues {
            history_window_days: page.history_window_days,
            dedup_window_days: page.dedup_window_days,
            max_text_bytes: page.max_text_bytes,
        });
    }

    pub(super) fn history_window_input(&self) -> Entity<InputState> {
        self.history_window_input.clone()
    }

    pub(super) fn dedup_window_input(&self) -> Entity<InputState> {
        self.dedup_window_input.clone()
    }

    pub(super) fn max_text_bytes_input(&self) -> Entity<InputState> {
        self.max_text_bytes_input.clone()
    }

    pub(super) fn history_window_unit(&self) -> StorageDurationUnit {
        self.history_window_unit
    }

    pub(super) fn dedup_window_unit(&self) -> StorageDurationUnit {
        self.dedup_window_unit
    }

    pub(super) fn max_text_bytes_unit(&self) -> StorageBytesUnit {
        self.max_text_bytes_unit
    }

    pub(super) fn select_history_window_unit(
        &mut self,
        next_unit: StorageDurationUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        Self::switch_duration_unit(
            &self.history_window_input,
            &mut self.history_window_unit,
            next_unit,
            window,
            cx,
        );
    }

    pub(super) fn select_dedup_window_unit(
        &mut self,
        next_unit: StorageDurationUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        Self::switch_duration_unit(
            &self.dedup_window_input,
            &mut self.dedup_window_unit,
            next_unit,
            window,
            cx,
        );
    }

    pub(super) fn select_max_text_bytes_unit(
        &mut self,
        next_unit: StorageBytesUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        Self::switch_bytes_unit(
            &self.max_text_bytes_input,
            &mut self.max_text_bytes_unit,
            next_unit,
            window,
            cx,
        );
    }

    pub(super) fn history_window_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.history_window_input.read(cx).value().to_string()
    }

    pub(super) fn dedup_window_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.dedup_window_input.read(cx).value().to_string()
    }

    pub(super) fn max_text_bytes_value(&self, cx: &Context<WorkspaceView>) -> String {
        self.max_text_bytes_input.read(cx).value().to_string()
    }

    pub(super) fn history_window_days(
        &self,
        cx: &Context<WorkspaceView>,
    ) -> Result<u32, String> {
        parse_duration_value(
            &self.history_window_value(cx),
            self.history_window_unit,
            "History window",
        )
    }

    pub(super) fn dedup_window_days(
        &self,
        cx: &Context<WorkspaceView>,
    ) -> Result<u32, String> {
        parse_duration_value(
            &self.dedup_window_value(cx),
            self.dedup_window_unit,
            "Dedup window",
        )
    }

    pub(super) fn max_text_bytes(&self, cx: &Context<WorkspaceView>) -> Result<usize, String> {
        parse_bytes_value(
            &self.max_text_bytes_value(cx),
            self.max_text_bytes_unit,
            "Max text bytes",
        )
    }

    fn sync_duration_input_if_clean(
        &self,
        entity: &Entity<InputState>,
        previous_days: u32,
        next_days: u32,
        unit: StorageDurationUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        if parse_duration_value(&entity.read(cx).value().to_string(), unit, "Storage field")
            .is_ok_and(|current_days| current_days == previous_days)
        {
            self.set_input_value(entity, format_duration_value(next_days, unit), window, cx);
        }
    }

    fn sync_bytes_input_if_clean(
        &self,
        entity: &Entity<InputState>,
        previous_bytes: usize,
        next_bytes: usize,
        unit: StorageBytesUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        if parse_bytes_value(&entity.read(cx).value().to_string(), unit, "Storage field")
            .is_ok_and(|current_bytes| current_bytes == previous_bytes)
        {
            self.set_input_value(entity, format_bytes_value(next_bytes, unit), window, cx);
        }
    }

    fn set_input_value(
        &self,
        entity: &Entity<InputState>,
        value: String,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        entity.update(cx, |input, cx| {
            input.set_value(value.clone(), window, cx);
        });
    }

    fn switch_duration_unit(
        entity: &Entity<InputState>,
        current_unit: &mut StorageDurationUnit,
        next_unit: StorageDurationUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        if *current_unit == next_unit {
            return;
        }

        let parsed_days =
            parse_duration_value(&entity.read(cx).value().to_string(), *current_unit, "Storage field")
                .ok();
        *current_unit = next_unit;

        if let Some(days) = parsed_days {
            entity.update(cx, |input, cx| {
                input.set_value(format_duration_value(days, next_unit), window, cx);
            });
        }
    }

    fn switch_bytes_unit(
        entity: &Entity<InputState>,
        current_unit: &mut StorageBytesUnit,
        next_unit: StorageBytesUnit,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        if *current_unit == next_unit {
            return;
        }

        let parsed_bytes =
            parse_bytes_value(&entity.read(cx).value().to_string(), *current_unit, "Storage field")
                .ok();
        *current_unit = next_unit;

        if let Some(bytes) = parsed_bytes {
            entity.update(cx, |input, cx| {
                input.set_value(format_bytes_value(bytes, next_unit), window, cx);
            });
        }
    }
}

fn parse_duration_value(
    raw: &str,
    unit: StorageDurationUnit,
    label: &str,
) -> Result<u32, String> {
    let amount = parse_positive_decimal(raw, label)?;
    let days = unit.to_days(amount).round();
    if !days.is_finite() || days < 1.0 || days > f64::from(u32::MAX) {
        return Err(format!("{label} is out of range."));
    }
    Ok(days as u32)
}

fn parse_bytes_value(raw: &str, unit: StorageBytesUnit, label: &str) -> Result<usize, String> {
    let amount = parse_positive_decimal(raw, label)?;
    let bytes = (amount * unit.factor()).round();
    if !bytes.is_finite() || bytes < 1.0 || bytes > usize::MAX as f64 {
        return Err(format!("{label} is out of range."));
    }
    Ok(bytes as usize)
}

fn parse_positive_decimal(raw: &str, label: &str) -> Result<f64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} cannot be empty."));
    }

    let amount = trimmed
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a valid number."))?;
    if !amount.is_finite() || amount <= 0.0 {
        return Err(format!("{label} must be greater than zero."));
    }

    Ok(amount)
}

fn format_duration_value(days: u32, unit: StorageDurationUnit) -> String {
    format_scaled_value(unit.value_from_days(days))
}

fn format_bytes_value(bytes: usize, unit: StorageBytesUnit) -> String {
    format_scaled_value(unit.value_from_bytes(bytes))
}

fn format_scaled_value(value: f64) -> String {
    let rounded = (value * 10_000.0).round() / 10_000.0;
    let mut text = format!("{rounded:.4}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}
