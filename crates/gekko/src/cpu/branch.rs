#[derive(Debug)]
pub enum BranchControl {
    BranchIfConditionTrue,
    BranchIfConditionFalse,
    BranchAlways,
    DecrementBranchIfNotZeroAndConditionFalse,
    DecrementBranchIfZeroAndConditionFalse,
    DecrementBranchIfNotZeroAndConditionTrue,
    DecrementBranchIfZeroAndConditionTrue,
    DecrementBranchIfNotZero,
    DecrementBranchIfZero,
}

impl BranchControl {
    pub fn from_bo(value: u8) -> Self {
        let branch_hint = value & 0b1 == 0;
        tracing::trace!("Branch hint: {branch_hint}");

        match value & 0b11110 {
            0b00000 => Self::DecrementBranchIfNotZeroAndConditionFalse,
            0b00010 => Self::DecrementBranchIfZeroAndConditionFalse,
            0b00100 | 0b00110 => Self::BranchIfConditionFalse,
            0b01000 => Self::DecrementBranchIfNotZeroAndConditionTrue,
            0b01010 => Self::DecrementBranchIfZeroAndConditionTrue,
            0b01100 | 0b01110 => Self::BranchIfConditionTrue,
            0b10000 | 0b11000 => Self::DecrementBranchIfNotZero,
            0b10010 | 0b11010 => Self::DecrementBranchIfZero,
            _ => Self::BranchAlways,
        }
    }

    pub fn should_branch(&self, ctr: u32, condition: bool) -> bool {
        match self {
            Self::BranchIfConditionTrue => condition,
            Self::BranchIfConditionFalse => !condition,
            Self::BranchAlways => true,
            Self::DecrementBranchIfNotZeroAndConditionFalse => ctr != 0 && !condition,
            Self::DecrementBranchIfZeroAndConditionFalse => ctr == 0 && !condition,
            Self::DecrementBranchIfNotZeroAndConditionTrue => ctr != 0 && condition,
            Self::DecrementBranchIfZeroAndConditionTrue => ctr == 0 && condition,
            Self::DecrementBranchIfNotZero => ctr != 0,
            Self::DecrementBranchIfZero => ctr == 0,
        }
    }

    pub fn should_decrement_ctr(&self) -> bool {
        matches!(
            self,
            Self::DecrementBranchIfNotZeroAndConditionFalse
                | Self::DecrementBranchIfZeroAndConditionFalse
                | Self::DecrementBranchIfNotZeroAndConditionTrue
                | Self::DecrementBranchIfZeroAndConditionTrue
                | Self::DecrementBranchIfNotZero
                | Self::DecrementBranchIfZero
        )
    }
}
