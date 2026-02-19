use {
    crate::errors::SbpfVmError,
    std::{cell::RefCell, rc::Rc},
};

/// Compute meter for tracking and consuming compute units
#[derive(Debug, Clone)]
pub struct ComputeMeter {
    inner: Rc<RefCell<ComputeMeterInner>>,
}

impl ComputeMeter {
    pub fn new(limit: u64) -> Self {
        Self {
            inner: Rc::new(RefCell::new(ComputeMeterInner::new(limit))),
        }
    }

    pub fn consume(&self, amount: u64) -> Result<(), SbpfVmError> {
        self.inner.borrow_mut().consume(amount)
    }

    pub fn get_remaining(&self) -> u64 {
        self.inner.borrow().get_remaining()
    }

    pub fn get_consumed(&self) -> u64 {
        self.inner.borrow().consumed
    }

    pub fn reset(&self) {
        self.inner.borrow_mut().reset();
    }

    pub fn borrow(&self) -> std::cell::Ref<'_, ComputeMeterInner> {
        self.inner.borrow()
    }

    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, ComputeMeterInner> {
        self.inner.borrow_mut()
    }
}

#[derive(Debug)]
pub struct ComputeMeterInner {
    pub consumed: u64,
    pub limit: u64,
}

impl ComputeMeterInner {
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
