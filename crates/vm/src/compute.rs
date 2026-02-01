use crate::errors::SbpfVmError;

/// Compute meter for tracking and consuming compute units
pub struct ComputeMeter {
    pub consumed: u64,
    pub limit: u64,
}

impl ComputeMeter {
    pub fn new(limit: u64) -> Self {
        Self { consumed: 0, limit }
    }

    pub fn consume(&mut self, amount: u64) -> Result<(), SbpfVmError> {
        let new_total = self.consumed.saturating_add(amount);
        if new_total > self.limit {
            return Err(SbpfVmError::ComputeBudgetExceeded {
                limit: self.limit,
                consumed: new_total,
            });
        }
        self.consumed = new_total;
        Ok(())
    }

    pub fn get_remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }

    pub fn reset(&mut self) {
        self.consumed = 0;
    }
}
