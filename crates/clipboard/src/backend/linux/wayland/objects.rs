use super::events::{Payload, State};
use std::{os::fd::BorrowedFd, sync::Arc};
use wayland_client::{Proxy, QueueHandle, protocol::wl_seat::WlSeat};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as ed, ext_data_control_manager_v1 as em,
    ext_data_control_offer_v1 as eo, ext_data_control_source_v1 as es,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1 as wd, zwlr_data_control_manager_v1 as wm,
    zwlr_data_control_offer_v1 as wo, zwlr_data_control_source_v1 as ws,
};

pub(super) enum Manager {
    Ext(em::ExtDataControlManagerV1),
    Wlr(wm::ZwlrDataControlManagerV1),
}
impl Manager {
    pub fn device(&self, seat: &WlSeat, q: &QueueHandle<State>) -> Device {
        match self {
            Self::Ext(m) => Device::Ext(m.get_data_device(seat, q, ())),
            Self::Wlr(m) => Device::Wlr(m.get_data_device(seat, q, ())),
        }
    }
    pub fn source(&self, q: &QueueHandle<State>, data: Arc<Payload>) -> Source {
        match self {
            Self::Ext(m) => Source::Ext(m.create_data_source(q, data)),
            Self::Wlr(m) => Source::Wlr(m.create_data_source(q, data)),
        }
    }
}
pub(super) enum Device {
    Ext(ed::ExtDataControlDeviceV1),
    Wlr(wd::ZwlrDataControlDeviceV1),
}
impl Device {
    pub fn select(&self, source: &Source) {
        match (self, source) {
            (Self::Ext(d), Source::Ext(s)) => d.set_selection(Some(s)),
            (Self::Wlr(d), Source::Wlr(s)) => d.set_selection(Some(s)),
            _ => unreachable!(),
        }
    }
    pub fn destroy(&self) {
        match self {
            Self::Ext(d) => d.destroy(),
            Self::Wlr(d) => d.destroy(),
        }
    }
}
pub(super) enum Source {
    Ext(es::ExtDataControlSourceV1),
    Wlr(ws::ZwlrDataControlSourceV1),
}
impl Source {
    pub fn offer(&self, mime: &str) {
        match self {
            Self::Ext(s) => s.offer(mime.into()),
            Self::Wlr(s) => s.offer(mime.into()),
        }
    }
    pub fn destroy(&self) {
        match self {
            Self::Ext(s) => s.destroy(),
            Self::Wlr(s) => s.destroy(),
        }
    }
    pub fn id(&self) -> wayland_client::backend::ObjectId {
        match self {
            Self::Ext(s) => s.id(),
            Self::Wlr(s) => s.id(),
        }
    }
}
#[derive(Clone)]
pub(super) enum Offer {
    Ext(eo::ExtDataControlOfferV1),
    Wlr(wo::ZwlrDataControlOfferV1),
}
impl Offer {
    pub fn receive(&self, mime: &str, fd: BorrowedFd<'_>) {
        match self {
            Self::Ext(o) => o.receive(mime.into(), fd),
            Self::Wlr(o) => o.receive(mime.into(), fd),
        }
    }
    pub fn destroy(&self) {
        match self {
            Self::Ext(o) => o.destroy(),
            Self::Wlr(o) => o.destroy(),
        }
    }
    pub fn id(&self) -> wayland_client::backend::ObjectId {
        match self {
            Self::Ext(o) => o.id(),
            Self::Wlr(o) => o.id(),
        }
    }
}
