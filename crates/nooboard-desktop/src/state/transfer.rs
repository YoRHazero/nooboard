#[derive(Clone)]
pub struct TransferItem {
    pub id: String,
    pub file_name: String,
    pub size_label: String,
    pub source_device: String,
    pub status: TransferStatus,
}

impl TransferItem {
    pub fn awaiting_review(
        id: impl Into<String>,
        file_name: impl Into<String>,
        size_label: impl Into<String>,
        source_device: impl Into<String>,
        queued_at_label: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            file_name: file_name.into(),
            size_label: size_label.into(),
            source_device: source_device.into(),
            status: TransferStatus::AwaitingReview {
                queued_at_label: queued_at_label.into(),
            },
        }
    }

    pub fn start_transfer(
        self,
        progress: f32,
        speed_label: impl Into<String>,
        started_at_label: impl Into<String>,
        elapsed_label: impl Into<String>,
        eta_label: impl Into<String>,
    ) -> Self {
        match self.status {
            TransferStatus::AwaitingReview { .. } => Self {
                id: self.id,
                file_name: self.file_name,
                size_label: self.size_label,
                source_device: self.source_device,
                status: TransferStatus::Progress {
                    progress,
                    speed_label: speed_label.into(),
                    started_at_label: started_at_label.into(),
                    elapsed_label: elapsed_label.into(),
                    eta_label: eta_label.into(),
                },
            },
            _ => panic!("transfer item can only enter progress from awaiting review"),
        }
    }

    pub fn complete_transfer(
        self,
        completed_at_label: impl Into<String>,
        duration_label: impl Into<String>,
    ) -> Self {
        match self.status {
            TransferStatus::Progress { .. } => Self {
                id: self.id,
                file_name: self.file_name,
                size_label: self.size_label,
                source_device: self.source_device,
                status: TransferStatus::Complete {
                    completed_at_label: completed_at_label.into(),
                    duration_label: duration_label.into(),
                },
            },
            _ => panic!("transfer item can only complete from in-progress"),
        }
    }

    pub fn stage(&self) -> TransferStage {
        match &self.status {
            TransferStatus::AwaitingReview { .. } => TransferStage::AwaitingReview,
            TransferStatus::Progress { .. } => TransferStage::Progress,
            TransferStatus::Complete { .. } => TransferStage::Complete,
        }
    }

    pub fn is_awaiting_review(&self) -> bool {
        self.stage() == TransferStage::AwaitingReview
    }

    pub fn is_progress(&self) -> bool {
        self.stage() == TransferStage::Progress
    }

    pub fn is_complete(&self) -> bool {
        self.stage() == TransferStage::Complete
    }
}

#[derive(Clone)]
pub enum TransferStatus {
    AwaitingReview {
        queued_at_label: String,
    },
    Progress {
        progress: f32,
        speed_label: String,
        started_at_label: String,
        elapsed_label: String,
        eta_label: String,
    },
    Complete {
        completed_at_label: String,
        duration_label: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransferStage {
    AwaitingReview,
    Progress,
    Complete,
}
