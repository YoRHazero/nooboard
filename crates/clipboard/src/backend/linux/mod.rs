mod formats;
mod io;
mod wayland;
mod x11;
use crate::{BackendPreference, Error, Options, Result, backend::Backend};

pub(super) fn open(options: &Options) -> Result<Box<dyn Backend>> {
    let preference = match options.backend {
        BackendPreference::Automatic => match std::env::var("NOOBOARD_LINUX_BACKEND").as_deref() {
            Ok("x11") => BackendPreference::X11,
            Ok("wayland") => BackendPreference::Wayland,
            Ok(_) => return Err(Error::InvalidInput),
            Err(_) if std::env::var_os("WAYLAND_DISPLAY").is_some() => BackendPreference::Wayland,
            Err(_) if std::env::var_os("DISPLAY").is_some() => BackendPreference::X11,
            Err(_) => {
                return Err(Error::UnsupportedSession(
                    "no X11 or Wayland display; run in a desktop session",
                ));
            }
        },
        preference => preference,
    };
    match preference {
        BackendPreference::X11 => Ok(Box::new(x11::Clipboard::open(options)?)),
        BackendPreference::Wayland => Ok(Box::new(wayland::Clipboard::open(options)?)),
        BackendPreference::Automatic => unreachable!("automatic backend was resolved"),
    }
}
