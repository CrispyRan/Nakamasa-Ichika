use std::marker::PhantomData;
use std::sync::atomic::{AtomicPtr, Ordering};
use once_cell::sync::OnceCell;

/// 零成本配置访问器
pub struct ConfigAccessor<T> {
    _marker: PhantomData<T>,
}

impl<T> ConfigAccessor<T> {
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
    
    /// 编译时常量配置访问
    pub const fn get(&self) -> &'static T {
        static CONFIG: OnceCell<T> = OnceCell::new();
        CONFIG.get().expect("Config not initialized")
    }
}

/// 无锁配置更新器
pub struct AtomicConfig<T> {
    ptr: AtomicPtr<T>,
}

impl<T> AtomicConfig<T> {
    pub fn new(initial: T) -> Self {
        let boxed = Box::new(initial);
        Self {
            ptr: AtomicPtr::new(Box::into_raw(boxed)),
        }
    }
    
    /// 无锁读取配置
    pub fn load(&self) -> &T {
        unsafe { &*self.ptr.load(Ordering::Acquire) }
    }
    
    /// 原子更新配置
    pub fn store(&self, new_value: T) {
        let new_ptr = Box::into_raw(Box::new(new_value));
        let old_ptr = self.ptr.swap(new_ptr, Ordering::AcqRel);
        unsafe { drop(Box::from_raw(old_ptr)) };
    }
}

unsafe impl<T: Send> Send for AtomicConfig<T> {}
unsafe impl<T: Sync> Sync for AtomicConfig<T> {}
