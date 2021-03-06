//! An `OpenCL` platform identifier.
//!
//! Documentation copied from [https://www.khronos.org/registry/cl/sdk/1.2/doc
//! s/man/xhtml/clGetPlatformInfo.html](https://www.khronos.org/registry/cl/sd
//! k/1.2/docs/man/xhtml/clGetPlatformInfo.html)

use std;
use std::ops::{Deref, DerefMut};
use ffi::cl_platform_id;
use core::{self, PlatformId as PlatformIdCore, PlatformInfo, PlatformInfoResult, ClPlatformIdPtr};
use core::error::{Result as OclCoreResult};

/// A platform identifier.
///
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Platform(PlatformIdCore);

impl Platform {
    /// Returns a list of all platforms avaliable on the host machine.
    pub fn list() -> Vec<Platform> {
        let list_core = core::get_platform_ids()
            .expect("Platform::list: Error retrieving platform list");

        list_core.into_iter().map(Platform::new).collect()
    }

    /// Returns the first available platform.
    ///
    /// If `ignore_env_var` is set to `true`, the `OCL_DEFAULT_PLATFORM_IDX`
    /// environment variable will be ignored and the platform with index 0
    /// returned from `core::get_platform_ids()` will be returned.
    ///
    /// This method differs from `Platform::default()` in two ways. First, it
    /// optionally ignores the `OCL_DEFAULT_PLATFORM_IDX` environment variable
    /// (`Platform::default` always respects it). Second, this function will
    /// not panic if no platforms are available and will return an error
    /// instead.
    pub fn first(ignore_env_var: bool) -> OclCoreResult<Platform> {
        if ignore_env_var {
            Ok(Platform::new(core::get_platform_ids()?[0]))
        } else {
            Ok(Platform::new(core::default_platform()?))
        }
    }

    /// Creates a new `Platform` from a `PlatformIdCore`.
    ///
    /// ## Safety
    ///
    /// Not meant to be called unless you know what you're doing.
    ///
    /// Use list to get a list of platforms.
    pub fn new(id_core: PlatformIdCore) -> Platform {
        Platform(id_core)
    }

    /// Returns a list of `Platform`s from a list of `PlatformIdCore`s
    pub fn list_from_core(platforms: Vec<PlatformIdCore>) -> Vec<Platform> {
        platforms.into_iter().map(Platform::new).collect()
    }

    /// Returns info about the platform.
    pub fn info(&self, info_kind: PlatformInfo) -> OclCoreResult<PlatformInfoResult> {
        core::get_platform_info(&self.0, info_kind)
    }

    /// Returns the platform profile as a string.
    ///
    /// Returns the profile name supported by the implementation. The profile
    /// name returned can be one of the following strings:
    ///
    /// * FULL_PROFILE - if the implementation supports the OpenCL
    ///   specification (functionality defined as part of the core
    ///   specification and does not require any extensions to be supported).
    ///
    /// * EMBEDDED_PROFILE - if the implementation supports the OpenCL
    ///   embedded profile. The embedded profile is defined to be a subset for
    ///   each version of OpenCL.
    ///
    pub fn profile(&self) -> OclCoreResult<String> {
        core::get_platform_info(&self.0, PlatformInfo::Profile).map(|r| r.into())
    }

    /// Returns the platform driver version as a string.
    ///
    /// Returns the OpenCL version supported by the implementation. This
    /// version string has the following format:
    ///
    /// * OpenCL<space><major_version.minor_version><space><platform-specific
    ///   information>
    ///
    /// * The major_version.minor_version value returned will be '1.2'.
    ///
    /// * TODO: Convert this to new version system returning an `OpenclVersion`.
    pub fn version(&self) -> OclCoreResult<String> {
        core::get_platform_info(&self.0, PlatformInfo::Version).map(|r| r.into())
    }

    /// Returns the platform name as a string.
    pub fn name(&self) -> OclCoreResult<String> {
        core::get_platform_info(&self.0, PlatformInfo::Name).map(|r| r.into())
    }

    /// Returns the platform vendor as a string.
    pub fn vendor(&self) -> OclCoreResult<String> {
        core::get_platform_info(&self.0, PlatformInfo::Vendor).map(|r| r.into())
    }

    /// Returns the list of platform extensions as a string.
    ///
    /// Returns a space-separated list of extension names (the extension names
    /// themselves do not contain any spaces) supported by the platform.
    /// Extensions defined here must be supported by all devices associated
    /// with this platform.
    pub fn extensions(&self) -> OclCoreResult<String> {
        core::get_platform_info(&self.0, PlatformInfo::Extensions).map(|r| r.into())
    }

    /// Returns a reference to the underlying `PlatformIdCore`.
    pub fn as_core(&self) -> &PlatformIdCore {
        &self.0
    }

    fn fmt_info(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Platform")
            .field("Profile", &self.info(PlatformInfo::Profile))
            .field("Version", &self.info(PlatformInfo::Version))
            .field("Name", &self.info(PlatformInfo::Name))
            .field("Vendor", &self.info(PlatformInfo::Vendor))
            .field("Extensions", &self.info(PlatformInfo::Extensions))
            .finish()
    }
}

unsafe impl ClPlatformIdPtr for Platform {
    fn as_ptr(&self) -> cl_platform_id {
        self.0.as_ptr()
    }
}
// unsafe impl<'a> ClPlatformIdPtr for &'a Platform {}

impl Default for Platform {
    /// Returns the first (0th) platform available, or the platform specified
    /// by the `OCL_DEFAULT_PLATFORM_IDX` environment variable if it is set.
    ///
    /// ### Panics
    ///
    /// Panics upon any OpenCL API error.
    ///
    fn default() -> Platform {
        let dflt_plat_core = core::default_platform().expect("Platform::default()");
        Platform::new(dflt_plat_core)
    }
}

impl From<PlatformIdCore> for Platform {
    fn from(core: PlatformIdCore) -> Platform {
        Platform(core)
    }
}

impl From<Platform> for String {
    fn from(p: Platform) -> String {
        format!("{}", p)
    }
}

impl From<Platform> for PlatformIdCore {
    fn from(p: Platform) -> PlatformIdCore {
        p.0
    }
}

impl<'a> From<&'a Platform> for PlatformIdCore {
    fn from(p: &Platform) -> PlatformIdCore {
        p.0.clone()
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.fmt_info(f)
    }
}


impl Deref for Platform {
    type Target = PlatformIdCore;

    fn deref(&self) -> &PlatformIdCore {
        &self.0
    }
}

impl DerefMut for Platform {
    fn deref_mut(&mut self) -> &mut PlatformIdCore {
        &mut self.0
    }
}
