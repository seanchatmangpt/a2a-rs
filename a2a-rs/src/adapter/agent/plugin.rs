#[cfg(feature = "hotreload")]
use std::ffi::{CStr, CString};
#[cfg(feature = "hotreload")]
use std::os::raw::c_char;

#[cfg(feature = "hotreload")]
use crate::domain::{A2AError, Message, Task};

#[cfg(feature = "hotreload")]
pub const PLUGIN_API_VERSION: u32 = 1;

#[cfg(feature = "hotreload")]
#[repr(C)]
pub struct PluginMetadata {
    pub api_version: u32,
    pub plugin_version: *const c_char,
    pub plugin_name: *const c_char,
    pub plugin_author: *const c_char,
}

#[cfg(feature = "hotreload")]
#[repr(C)]
pub struct CMessage {
    pub json_data: *const c_char,
}

#[cfg(feature = "hotreload")]
#[repr(C)]
pub struct CTask {
    pub json_data: *const c_char,
}

#[cfg(feature = "hotreload")]
#[repr(C)]
pub struct CError {
    pub message: *const c_char,
    pub code: i32,
}

#[cfg(feature = "hotreload")]
pub type ProcessMessageFn = unsafe extern "C" fn(
    task_id: *const c_char,
    message: *const CMessage,
    session_id: *const c_char,
    out_task: *mut *mut CTask,
    out_error: *mut *mut CError,
) -> bool;

#[cfg(feature = "hotreload")]
pub type ValidateMessageFn =
    unsafe extern "C" fn(message: *const CMessage, out_error: *mut *mut CError) -> bool;

#[cfg(feature = "hotreload")]
pub type GetMetadataFn = unsafe extern "C" fn() -> *const PluginMetadata;

#[cfg(feature = "hotreload")]
pub type FreeCStringFn = unsafe extern "C" fn(ptr: *mut c_char);

#[cfg(feature = "hotreload")]
pub type FreeTaskFn = unsafe extern "C" fn(task: *mut CTask);

#[cfg(feature = "hotreload")]
pub type FreeErrorFn = unsafe extern "C" fn(error: *mut CError);

#[cfg(feature = "hotreload")]
impl CMessage {
    pub unsafe fn from_message(msg: &Message) -> Result<Self, A2AError> {
        let json = serde_json::to_string(msg)?;
        let c_str = CString::new(json).map_err(|e| {
            A2AError::Internal(format!("Failed to create C string: {}", e))
        })?;
        Ok(Self {
            json_data: c_str.into_raw(),
        })
    }

    pub unsafe fn to_message(&self) -> Result<Message, A2AError> {
        if self.json_data.is_null() {
            return Err(A2AError::Internal("Null json_data pointer".to_string()));
        }
        let c_str = CStr::from_ptr(self.json_data);
        let json_str = c_str.to_str().map_err(|e| {
            A2AError::Internal(format!("Invalid UTF-8 in json_data: {}", e))
        })?;
        serde_json::from_str(json_str).map_err(|e| A2AError::JsonParse(e))
    }

    pub unsafe fn free(&mut self) {
        if !self.json_data.is_null() {
            drop(CString::from_raw(self.json_data as *mut c_char));
            self.json_data = std::ptr::null();
        }
    }
}

#[cfg(feature = "hotreload")]
impl CTask {
    pub unsafe fn from_task(task: &Task) -> Result<Self, A2AError> {
        let json = serde_json::to_string(task)?;
        let c_str = CString::new(json).map_err(|e| {
            A2AError::Internal(format!("Failed to create C string: {}", e))
        })?;
        Ok(Self {
            json_data: c_str.into_raw(),
        })
    }

    pub unsafe fn to_task(&self) -> Result<Task, A2AError> {
        if self.json_data.is_null() {
            return Err(A2AError::Internal("Null json_data pointer".to_string()));
        }
        let c_str = CStr::from_ptr(self.json_data);
        let json_str = c_str.to_str().map_err(|e| {
            A2AError::Internal(format!("Invalid UTF-8 in json_data: {}", e))
        })?;
        serde_json::from_str(json_str).map_err(|e| A2AError::JsonParse(e))
    }

    pub unsafe fn free(&mut self) {
        if !self.json_data.is_null() {
            drop(CString::from_raw(self.json_data as *mut c_char));
            self.json_data = std::ptr::null();
        }
    }
}

#[cfg(feature = "hotreload")]
impl CError {
    pub unsafe fn from_error(error: &A2AError) -> Result<Self, A2AError> {
        let message = error.to_string();
        let c_str = CString::new(message).map_err(|e| {
            A2AError::Internal(format!("Failed to create C string: {}", e))
        })?;
        let code = match error {
            A2AError::JsonParse(_) => -32700,
            A2AError::InvalidRequest(_) => -32600,
            A2AError::MethodNotFound(_) => -32601,
            A2AError::InvalidParams(_) => -32602,
            A2AError::TaskNotFound(_) => -32001,
            A2AError::TaskNotCancelable(_) => -32002,
            _ => -32603,
        };
        Ok(Self {
            message: c_str.into_raw(),
            code,
        })
    }

    pub unsafe fn to_error(&self) -> A2AError {
        if self.message.is_null() {
            return A2AError::Internal("Null error message pointer".to_string());
        }
        let c_str = CStr::from_ptr(self.message);
        let message = c_str.to_string_lossy().to_string();
        A2AError::Internal(format!("Plugin error ({}): {}", self.code, message))
    }

    pub unsafe fn free(&mut self) {
        if !self.message.is_null() {
            drop(CString::from_raw(self.message as *mut c_char));
            self.message = std::ptr::null();
        }
    }
}

#[cfg(feature = "hotreload")]
impl Drop for CMessage {
    fn drop(&mut self) {
        unsafe {
            self.free();
        }
    }
}

#[cfg(feature = "hotreload")]
impl Drop for CTask {
    fn drop(&mut self) {
        unsafe {
            self.free();
        }
    }
}

#[cfg(feature = "hotreload")]
impl Drop for CError {
    fn drop(&mut self) {
        unsafe {
            self.free();
        }
    }
}

#[cfg(feature = "hotreload")]
pub trait PluginExports {
    unsafe fn get_metadata() -> *const PluginMetadata;
    unsafe fn process_message(
        task_id: *const c_char,
        message: *const CMessage,
        session_id: *const c_char,
        out_task: *mut *mut CTask,
        out_error: *mut *mut CError,
    ) -> bool;
    unsafe fn validate_message(message: *const CMessage, out_error: *mut *mut CError) -> bool;
    unsafe fn free_c_string(ptr: *mut c_char);
    unsafe fn free_task(task: *mut CTask);
    unsafe fn free_error(error: *mut CError);
}

#[cfg(feature = "hotreload")]
#[macro_export]
macro_rules! export_plugin {
    ($handler:ty) => {
        use std::ffi::{CStr, CString};
        use std::os::raw::c_char;

        static METADATA: $crate::adapter::agent::plugin::PluginMetadata = $crate::adapter::agent::plugin::PluginMetadata {
            api_version: $crate::adapter::agent::plugin::PLUGIN_API_VERSION,
            plugin_version: concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char,
            plugin_name: concat!(env!("CARGO_PKG_NAME"), "\0").as_ptr() as *const c_char,
            plugin_author: concat!(env!("CARGO_PKG_AUTHORS"), "\0").as_ptr() as *const c_char,
        };

        #[no_mangle]
        pub unsafe extern "C" fn get_metadata() -> *const $crate::adapter::agent::plugin::PluginMetadata {
            &METADATA
        }

        #[no_mangle]
        pub unsafe extern "C" fn process_message(
            task_id: *const c_char,
            message: *const $crate::adapter::agent::plugin::CMessage,
            session_id: *const c_char,
            out_task: *mut *mut $crate::adapter::agent::plugin::CTask,
            out_error: *mut *mut $crate::adapter::agent::plugin::CError,
        ) -> bool {
            if task_id.is_null() || message.is_null() || out_task.is_null() || out_error.is_null() {
                return false;
            }

            let task_id_str = match CStr::from_ptr(task_id).to_str() {
                Ok(s) => s,
                Err(_) => return false,
            };

            let session_id_opt = if session_id.is_null() {
                None
            } else {
                match CStr::from_ptr(session_id).to_str() {
                    Ok(s) => Some(s),
                    Err(_) => return false,
                }
            };

            let msg = match (*message).to_message() {
                Ok(m) => m,
                Err(e) => {
                    if let Ok(err) = $crate::adapter::agent::plugin::CError::from_error(&e) {
                        *out_error = Box::into_raw(Box::new(err));
                    }
                    return false;
                }
            };

            let handler = <$handler>::default();
            let result = <$handler as $crate::port::MessageHandler>::process_message(
                &handler,
                task_id_str,
                &msg,
                session_id_opt,
            );

            match result {
                Ok(task) => {
                    match $crate::adapter::agent::plugin::CTask::from_task(&task) {
                        Ok(c_task) => {
                            *out_task = Box::into_raw(Box::new(c_task));
                            true
                        }
                        Err(e) => {
                            if let Ok(err) = $crate::adapter::agent::plugin::CError::from_error(&e) {
                                *out_error = Box::into_raw(Box::new(err));
                            }
                            false
                        }
                    }
                }
                Err(e) => {
                    if let Ok(err) = $crate::adapter::agent::plugin::CError::from_error(&e) {
                        *out_error = Box::into_raw(Box::new(err));
                    }
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn validate_message(
            message: *const $crate::adapter::agent::plugin::CMessage,
            out_error: *mut *mut $crate::adapter::agent::plugin::CError,
        ) -> bool {
            if message.is_null() || out_error.is_null() {
                return false;
            }

            let msg = match (*message).to_message() {
                Ok(m) => m,
                Err(e) => {
                    if let Ok(err) = $crate::adapter::agent::plugin::CError::from_error(&e) {
                        *out_error = Box::into_raw(Box::new(err));
                    }
                    return false;
                }
            };

            let handler = <$handler>::default();
            match <$handler as $crate::port::MessageHandler>::validate_message(&handler, &msg) {
                Ok(_) => true,
                Err(e) => {
                    if let Ok(err) = $crate::adapter::agent::plugin::CError::from_error(&e) {
                        *out_error = Box::into_raw(Box::new(err));
                    }
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn free_c_string(ptr: *mut c_char) {
            if !ptr.is_null() {
                drop(CString::from_raw(ptr));
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn free_task(task: *mut $crate::adapter::agent::plugin::CTask) {
            if !task.is_null() {
                drop(Box::from_raw(task));
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn free_error(error: *mut $crate::adapter::agent::plugin::CError) {
            if !error.is_null() {
                drop(Box::from_raw(error));
            }
        }
    };
}
