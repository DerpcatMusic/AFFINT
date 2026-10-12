use std::mem::size_of;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::Error;

#[derive(Debug)]
pub(crate) struct Budget {
    used: AtomicUsize,
    limit: usize,
}

impl Budget {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            used: AtomicUsize::new(0),
            limit,
        }
    }

    pub(crate) fn used(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }

    pub(crate) fn reserve(self: &Arc<Self>, bytes: usize) -> Result<Lease, Error> {
        self.acquire(bytes)?;
        Ok(Lease {
            budget: Arc::clone(self),
            bytes,
        })
    }

    fn acquire(&self, bytes: usize) -> Result<(), Error> {
        let mut used = self.used();
        loop {
            let next = used
                .checked_add(bytes)
                .filter(|next| *next <= self.limit)
                .ok_or(Error::BudgetExceeded {
                    requested: bytes,
                    used,
                    limit: self.limit,
                })?;
            match self
                .used
                .compare_exchange_weak(used, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return Ok(()),
                Err(current) => used = current,
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Lease {
    budget: Arc<Budget>,
    bytes: usize,
}

impl Lease {
    pub(crate) fn split(&mut self, bytes: usize) -> Result<Self, Error> {
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or(Error::Overflow("allocation lease split"))?;
        Ok(Self {
            budget: Arc::clone(&self.budget),
            bytes,
        })
    }

    fn grow(&mut self, bytes: usize) -> Result<(), Error> {
        self.budget.acquire(bytes)?;
        self.bytes += bytes;
        Ok(())
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

// Fields drop in declaration order: allocation first, accounting second.
#[derive(Debug)]
pub(crate) struct Buffer<T> {
    pub(crate) data: Vec<T>,
    pub(crate) lease: Lease,
}

impl<T: Clone> Buffer<T> {
    pub(crate) fn new(
        budget: &Arc<Budget>,
        len: usize,
        value: T,
        resource: &'static str,
    ) -> Result<Self, Error> {
        let bytes = len
            .checked_mul(size_of::<T>())
            .ok_or(Error::Overflow(resource))?;
        let lease = budget.reserve(bytes)?;
        Self::with_lease(len, value, resource, lease)
    }

    pub(crate) fn with_lease(
        len: usize,
        value: T,
        resource: &'static str,
        mut lease: Lease,
    ) -> Result<Self, Error> {
        let bytes = len
            .checked_mul(size_of::<T>())
            .ok_or(Error::Overflow(resource))?;
        let mut data = Vec::new();
        data.try_reserve_exact(len)
            .map_err(|_| Error::AllocationFailed { resource, bytes })?;
        let actual = data
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(Error::Overflow(resource))?;
        if actual > bytes {
            lease.grow(actual - bytes)?;
        }
        data.resize(len, value);
        Ok(Self { data, lease })
    }
}
