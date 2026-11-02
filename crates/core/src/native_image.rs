use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_NATIVE_IMAGE_ID: AtomicU64 = AtomicU64::new(1 << 63);

#[derive(Clone)]
pub struct NativeImage {
    id: u64,
    resource: Arc<dyn Any + Send + Sync>,
}

impl NativeImage {
    pub fn new(resource: impl Any + Send + Sync) -> Self {
        Self {
            id: NEXT_NATIVE_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
            resource: Arc::new(resource),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn resource<T: Any>(&self) -> Option<&T> {
        self.resource.downcast_ref::<T>()
    }
}
