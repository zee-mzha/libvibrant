mod nvidia_controller;
mod ctm_controller;

use crate::instance::xwrapper::{RROutput, Display};
use x11::{xlib, xrandr};
use libXNVCtrl_sys as nvctrl;
use std::os::raw::{c_int, c_uchar};
use std::slice::from_raw_parts;
use std::ptr::null_mut;
pub use crate::instance::controller::nvidia_controller::NvidiaController;
pub use crate::instance::controller::ctm_controller::CTMController;
use std::ffi::CStr;

const SATURATION_MIN: f64 = 0.0;
const SATURATION_MAX: f64 = 4.0;

pub enum ControllerBackend {
    XNVCtrl,
    CTM
}

pub fn get_controllers(display: &Display) -> Vec<Box<dyn Controller>> {
    let outputs = RROutput::from_display(display);
    let mut controllers = Vec::<Box<dyn Controller>>::with_capacity(outputs.len());

    // (nvidia_id, xrandr_id)
    let mut nvidia_ids = Vec::new();
    if display.has_nvidia(){
        //this will give us the id nvidia assigns to each display and its respective xrandr id
        unsafe {
            for i in 0..xlib::XScreenCount(display.xcon()) {
                let mut ids: *mut c_int = null_mut();
                // not really going to use this
                let mut ids_len: c_int = 0;
                // The way this call works is weird, ids_len will contain how many bytes there are
                // in ids. The first element will of ids is going to contain how many elements are
                // in ids. That's why we eventually override the value in ids_len
                nvctrl::XNVCTRLQueryBinaryData(display.xcon(), i, 0,
                                               nvctrl::NV_CTRL_BINARY_DATA_DISPLAYS_ENABLED_ON_XSCREEN,
                                               &mut ids as *mut *mut c_int as *mut *mut c_uchar,
                                               &mut ids_len as *mut _);
                ids_len = *ids;
                let ids = from_raw_parts(ids, ids_len as usize);
                nvidia_ids.reserve(ids.len());
                for id in ids {
                    let mut xrandr_id = 0;
                    nvctrl::XNVCTRLQueryTargetAttribute(display.xcon(),
                                                        nvctrl::NV_CTRL_TARGET_TYPE_DISPLAY,
                                                        *id, 0,
                                                        nvctrl::NV_CTRL_DISPLAY_RANDR_OUTPUT_ID,
                                                        &mut xrandr_id as *mut u64 as *mut i32);

                    nvidia_ids.push((*id, xrandr_id));
                }
            }
        }
    }

    //check which outputs have CTM
    let prop_atom;
    unsafe {
        prop_atom = xlib::XInternAtom(display.xcon(),
                                      CStr::from_bytes_with_nul_unchecked(b"CTM\0").as_ptr(), 1);
    }

    'outer: for output in outputs {
        // Check if this output can be controlled by XNVCtrl and add it as such if so
        for (nvidia_id, xrandr_id) in &nvidia_ids {
            if output.id() == *xrandr_id {
                controllers.push(Box::new(NvidiaController::new(output, *nvidia_id)));
                continue 'outer;
            }
        }
        // If not then check if it can be controlled by CTM
        if prop_atom != 0 {
            let property_info;
            unsafe {
                property_info = xrandr::XRRQueryOutputProperty(display.xcon(), output.id(),
                                                               prop_atom);
            }

            if !property_info.is_null() {
                unsafe {
                    xlib::XFree(property_info as *mut _);
                }

                controllers.push(Box::new(CTMController::new(output, prop_atom)));
            }
        }
    }

    controllers
}

pub trait Controller {
    fn get_saturation(&self, xcon: &Display) -> f64;
    fn set_saturation(&self, xcon: &Display, saturation: f64);

    fn get_name(&self) -> &str;
    fn get_backend(&self) -> ControllerBackend;
}
