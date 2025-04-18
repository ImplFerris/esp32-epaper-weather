use chrono::{DateTime, Utc};
use defmt::info;
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
    Stack,
};
use heapless::{String, Vec};
use reqwless::{
    client::{HttpClient, TlsConfig},
    TlsReference,
};
use serde::Deserialize;
use serde_repr::Deserialize_repr;

const API_KEY: &str = env!("API_KEY");

pub struct WeatherApi {
    wifi: Stack<'static>,
    url: String<120>,
}

impl WeatherApi {
    pub fn new(wifi: Stack<'static>) -> Self {
        let mut url = String::new();
        url.push_str(
            "https://api.openweathermap.org/data/2.5/weather?q=London&units=metric&appid=",
        )
        .unwrap();
        url.push_str(API_KEY).unwrap();
        Self { wifi, url }
    }

    pub async fn access_website(&self, tls_reference: TlsReference<'_>) -> WeatherData {
        let dns = DnsSocket::new(self.wifi);
        let tcp_state = TcpClientState::<1, 4096, 4096>::new();
        let tcp = TcpClient::new(self.wifi, &tcp_state);
        let tls_config = TlsConfig::new(
            reqwless::TlsVersion::Tls1_2,
            reqwless::Certificates {
                ca_chain: reqwless::X509::pem(
                    concat!(include_str!("./ca_cert.pem"), "\0").as_bytes(),
                )
                .ok(),
                ..Default::default()
            },
            tls_reference,
        );

        let mut client = HttpClient::new_with_tls(&tcp, &dns, tls_config);
        let mut buffer = [0u8; 4096];
        let mut http_req = client
            .request(reqwless::request::Method::GET, &self.url)
            .await
            .unwrap();
        let response = http_req.send(&mut buffer).await.unwrap();

        info!("Got response");
        let res = response.body().read_to_end().await.unwrap();

        let (data, _): (WeatherData, _) = serde_json_core::de::from_slice(res).unwrap();
        data
    }

    #[allow(dead_code)]
    fn get_example_data(&self) -> WeatherData {
        let json_data = r#"
            {
                "coord": {
                    "lon": -0.1257,
                    "lat": 51.5085
                },
                "weather": [
                    {
                    "id": 800,
                    "main": "Clear",
                    "description": "clear sky",
                    "icon": "01n"
                    }
                ],
                "base": "stations",
                "main": {
                    "temp": 3.75,
                    "feels_like": 1.23,
                    "temp_min": 3.75,
                    "temp_max": 3.75,
                    "pressure": 1025,
                    "humidity": 83,
                    "sea_level": 1025,
                    "grnd_level": 1020
                },
                "visibility": 10000,
                "wind": {
                    "speed": 2.72,
                    "deg": 51,
                    "gust": 8.22
                },
                "clouds": {
                    "all": 9
                },
                "dt": 1743995436,
                "sys": {
                    "country": "GB",
                    "sunrise": 1744003317,
                    "sunset": 1744051371
                },
                "timezone": 3600,
                "id": 2643743,
                "name": "London",
                "cod": 200
            }
        // "#;
        let (data, _): (WeatherData, _) = serde_json_core::de::from_str(json_data).unwrap();
        data
    }
}

#[derive(Debug, Deserialize)]
pub struct WeatherData {
    // pub coord: Coord,
    pub weather: Vec<Weather, 4>,
    pub main: Main,
    // pub visibility: i32,
    pub wind: Wind,
    // pub rain: Option<Rain>,
    // pub clouds: Clouds,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub dt: DateTime<Utc>,
    // pub sys: Sys,
    // pub timezone: i32,
    pub name: String<20>,
}

#[derive(Debug, Deserialize_repr)]
#[repr(u16)]
pub enum ConditionCode {
    // Group 2xx: Thunderstorm
    ThunderstormWithLightRain = 200,
    ThunderstormWithRain = 201,
    ThunderstormWithHeavyRain = 202,
    LightThunderstorm = 210,
    Thunderstorm = 211,
    HeavyThunderstorm = 212,
    RaggedThunderstorm = 221,
    ThunderstormWithLightDrizzle = 230,
    ThunderstormWithDrizzle = 231,
    ThunderstormWithHeavyDrizzle = 232,

    // Group 3xx: Drizzle
    LightIntensityDrizzle = 300,
    Drizzle = 301,
    HeavyIntensityDrizzle = 302,
    LightIntensityDrizzleRain = 310,
    DrizzleRain = 311,
    HeavyIntensityDrizzleRain = 312,
    ShowerRainAndDrizzle = 313,
    HeavyShowerRainAndDrizzle = 314,
    ShowerDrizzle = 321,

    // Group 5xx: Rain
    LightRain = 500,
    ModerateRain = 501,
    HeavyIntensityRain = 502,
    VeryHeavyRain = 503,
    ExtremeRain = 504,
    FreezingRain = 511,
    LightIntensityShowerRain = 520,
    ShowerRain = 521,
    HeavyIntensityShowerRain = 522,
    RaggedShowerRain = 531,

    // Group 6xx: Snow
    LightSnow = 600,
    Snow = 601,
    HeavySnow = 602,
    Sleet = 611,
    LightShowerSleet = 612,
    ShowerSleet = 613,
    LightRainAndSnow = 615,
    RainAndSnow = 616,
    LightShowerSnow = 620,
    ShowerSnow = 621,
    HeavyShowerSnow = 622,

    // Group 7xx: Atmosphere
    Mist = 701,
    Smoke = 711,
    Haze = 721,
    SandDustWhirls = 731,
    Fog = 741,
    Sand = 751,
    Dust = 761,
    VolcanicAsh = 762,
    Squalls = 771,
    Tornado = 781,

    // Group 800: Clear
    ClearSky = 800,

    // Group 80x: Clouds
    FewClouds = 801,
    ScatteredClouds = 802,
    BrokenClouds = 803,
    OvercastClouds = 804,
}

impl ConditionCode {
    pub fn icon(&self) -> &'static str {
        match self {
            // Thunderstorm
            ConditionCode::ThunderstormWithLightRain
            | ConditionCode::ThunderstormWithRain
            | ConditionCode::ThunderstormWithHeavyRain
            | ConditionCode::LightThunderstorm
            | ConditionCode::Thunderstorm
            | ConditionCode::HeavyThunderstorm
            | ConditionCode::RaggedThunderstorm
            | ConditionCode::ThunderstormWithLightDrizzle
            | ConditionCode::ThunderstormWithDrizzle
            | ConditionCode::ThunderstormWithHeavyDrizzle => "storm.bmp",

            // Drizzle
            ConditionCode::LightIntensityDrizzle
            | ConditionCode::Drizzle
            | ConditionCode::HeavyIntensityDrizzle
            | ConditionCode::LightIntensityDrizzleRain
            | ConditionCode::DrizzleRain
            | ConditionCode::HeavyIntensityDrizzleRain
            | ConditionCode::ShowerRainAndDrizzle
            | ConditionCode::HeavyShowerRainAndDrizzle
            | ConditionCode::ShowerDrizzle => "rainy.bmp",

            // Rain
            ConditionCode::LightRain
            | ConditionCode::ModerateRain
            | ConditionCode::HeavyIntensityRain
            | ConditionCode::VeryHeavyRain
            | ConditionCode::ExtremeRain
            | ConditionCode::LightIntensityShowerRain
            | ConditionCode::ShowerRain
            | ConditionCode::HeavyIntensityShowerRain
            | ConditionCode::RaggedShowerRain => "rainy_heavy.bmp",
            ConditionCode::FreezingRain => "weather_mix.bmp",

            // Snow
            ConditionCode::LightSnow
            | ConditionCode::Snow
            | ConditionCode::HeavySnow
            | ConditionCode::Sleet
            | ConditionCode::LightShowerSleet
            | ConditionCode::ShowerSleet
            | ConditionCode::LightRainAndSnow
            | ConditionCode::RainAndSnow
            | ConditionCode::LightShowerSnow
            | ConditionCode::ShowerSnow
            | ConditionCode::HeavyShowerSnow => "snowing.bmp",

            // Atmosphere
            ConditionCode::Mist
            | ConditionCode::Smoke
            | ConditionCode::Haze
            | ConditionCode::SandDustWhirls
            | ConditionCode::Fog
            | ConditionCode::Sand
            | ConditionCode::Dust
            | ConditionCode::VolcanicAsh
            | ConditionCode::Squalls => "foggy.bmp",
            ConditionCode::Tornado => "cyclone.bmp",

            // Clear
            ConditionCode::ClearSky => "sunny.bmp",

            // Clouds
            ConditionCode::FewClouds
            | ConditionCode::ScatteredClouds
            | ConditionCode::BrokenClouds
            | ConditionCode::OvercastClouds => "partly_cloudy_day.bmp",
        }
    }
}

// #[derive(Debug, Deserialize)]
// struct Coord {
//     lon: f64,
//     lat: f64,
// }

#[derive(Debug, Deserialize)]
pub struct Weather {
    // main: String<20>,
    // description: String<64>,
    pub id: ConditionCode,
    // icon: String<10>,
}

#[derive(Debug, Deserialize)]
pub struct Main {
    pub temp: f64,
    pub feels_like: f64,
    pub temp_min: f64,
    pub temp_max: f64,
    pub pressure: i32,
    pub humidity: i32,
    pub sea_level: Option<i32>,
    pub grnd_level: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Wind {
    pub speed: f64,
    pub deg: f64,
    pub gust: Option<f64>,
}

// #[derive(Debug, Deserialize)]
// struct Rain {
//     #[serde(rename = "1h")]
//     one_hour: f64,
// }

// #[derive(Debug, Deserialize)]
// struct Clouds {
//     all: i32,
// }

// #[derive(Debug, Deserialize)]
// struct Sys {
//     country: String<6>,
//     sunrise: i64,
//     sunset: i64,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     type_: Option<i32>,
//     id: Option<i32>,
// }
