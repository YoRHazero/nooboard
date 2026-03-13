fn main() {
    println!("cargo:rerun-if-changed=../../icons/nooboard-no-bkg.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut resource = winres::WindowsResource::new();
    resource.set_icon("../../icons/nooboard-no-bkg.ico");
    resource
        .compile()
        .expect("windows desktop icon resource must compile");
}
