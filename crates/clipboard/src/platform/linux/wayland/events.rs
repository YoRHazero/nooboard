use super::{
    objects::{Offer, Source},
    transfer::Writer,
};
use std::{collections::HashMap, sync::Arc};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    backend::ObjectId,
    globals::GlobalListContents,
    protocol::{wl_registry, wl_seat},
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as ed, ext_data_control_manager_v1 as em,
    ext_data_control_offer_v1 as eo, ext_data_control_source_v1 as es,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1 as wd, zwlr_data_control_manager_v1 as wm,
    zwlr_data_control_offer_v1 as wo, zwlr_data_control_source_v1 as ws,
};

pub(super) struct Payload {
    pub bytes: Arc<Vec<u8>>,
    pub marker: String,
}
pub(super) struct Offered {
    pub proxy: Offer,
    pub types: Vec<String>,
    pub overflow: bool,
}
#[derive(Default)]
pub(super) struct State {
    pub offers: HashMap<ObjectId, Offered>,
    pub selection: Option<ObjectId>,
    pub revision: u64,
    pub finished: bool,
    pub source: Option<Source>,
    pub payload: Option<Arc<Payload>>,
    pub writers: Vec<Writer>,
}
impl State {
    fn add_offer(&mut self, proxy: Offer) {
        // A compositor violating the protocol must not grow memory without a bound.
        if self.offers.len() >= 32 {
            proxy.destroy();
            self.finished = true;
            return;
        }
        self.offers.insert(
            proxy.id(),
            Offered {
                proxy,
                types: Vec::new(),
                overflow: false,
            },
        );
    }
    fn select(&mut self, id: Option<ObjectId>) {
        if let Some(old) = self.selection.take().filter(|old| Some(old) != id.as_ref()) {
            self.remove(&old);
        }
        self.selection = id;
        self.revision = self.revision.wrapping_add(1);
    }
    fn remove(&mut self, id: &ObjectId) {
        if let Some(offer) = self.offers.remove(id) {
            offer.proxy.destroy();
        }
    }
    fn mime(&mut self, id: ObjectId, mime: String) {
        if let Some(offer) = self.offers.get_mut(&id) {
            if offer.types.len() >= 256 || mime.len() > 1024 {
                offer.overflow = true;
            } else {
                offer.types.push(mime);
            }
        }
    }
    fn cancelled(&mut self, id: ObjectId) {
        if self.source.as_ref().is_some_and(|s| s.id() == id) {
            if let Some(source) = self.source.take() {
                source.destroy();
            }
            self.payload = None;
        }
    }
}

macro_rules! dispatch_protocol {
    ($device:ident, $device_ty:ident, $offer:ident, $offer_ty:ident, $source:ident, $source_ty:ident, $variant:ident) => {
        impl Dispatch<$device::$device_ty, ()> for State {
            fn event(state: &mut Self, _: &$device::$device_ty, event: $device::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                match event {
                    $device::Event::DataOffer { id } => state.add_offer(Offer::$variant(id)),
                    $device::Event::Selection { id } => state.select(id.map(|o| o.id())),
                    $device::Event::PrimarySelection { id: Some(id) } => state.remove(&id.id()),
                    $device::Event::Finished => state.finished = true,
                    _ => {}
                }
            }
            wayland_client::event_created_child!(State, $device::$device_ty, [0 => ($offer::$offer_ty, ())]);
        }
        impl Dispatch<$offer::$offer_ty, ()> for State {
            fn event(state: &mut Self, proxy: &$offer::$offer_ty, event: $offer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                if let $offer::Event::Offer { mime_type } = event { state.mime(proxy.id(), mime_type); }
            }
        }
        impl Dispatch<$source::$source_ty, Arc<Payload>> for State {
            fn event(state: &mut Self, proxy: &$source::$source_ty, event: $source::Event, data: &Arc<Payload>, _: &Connection, _: &QueueHandle<Self>) {
                match event {
                    $source::Event::Send { mime_type, fd } => {
                        if state.writers.len() < 8 && (super::super::formats::UTF8_TYPES.contains(&mime_type.as_str()) || mime_type == data.marker)
                            && let Some(writer) = Writer::new(fd, data.bytes.clone()) { state.writers.push(writer); }
                    }
                    $source::Event::Cancelled => state.cancelled(proxy.id()),
                    _ => {}
                }
            }
        }
    }
}
dispatch_protocol!(
    ed,
    ExtDataControlDeviceV1,
    eo,
    ExtDataControlOfferV1,
    es,
    ExtDataControlSourceV1,
    Ext
);
dispatch_protocol!(
    wd,
    ZwlrDataControlDeviceV1,
    wo,
    ZwlrDataControlOfferV1,
    ws,
    ZwlrDataControlSourceV1,
    Wlr
);
wayland_client::delegate_noop!(State: ignore em::ExtDataControlManagerV1);
wayland_client::delegate_noop!(State: ignore wm::ZwlrDataControlManagerV1);
wayland_client::delegate_noop!(State: ignore wl_seat::WlSeat);
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
