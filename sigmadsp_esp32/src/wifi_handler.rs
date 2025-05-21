use anyhow::{bail, Result};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripheral,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use log::info;

pub fn my_wifi(
    modem: impl peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
) -> Result<Box<EspWifi<'static>>> {
    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), None)?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    wifi.set_configuration(&Configuration::AccessPoint(
        esp_idf_svc::wifi::AccessPointConfiguration {
            ssid: "ESP32_SIGMADSP".try_into().unwrap(),
            password: "123456789".try_into().unwrap(),
            auth_method: AuthMethod::WPA2Personal,
            ..Default::default()
        },
    ))?;

    info!("Starting wifi...");

    wifi.start()?;

    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;
    info!("Wifi info: {ip_info:?}");

    Ok(Box::new(esp_wifi))
}
