extern crate reqwest;
extern crate hyper;
extern crate xdr_codec;

#[derive(Debug)]
pub enum Error {
    Unauthorized,
    InvalidCredentials,
    ChannelNotFound,
    InvalidParameters,
    UnknownStatus(reqwest::StatusCode),
    HTTPError(reqwest::Error),
}

type SCResult<T> = Result<T, Error>;

pub struct Point {
    pub timestamp: u64,
    pub value: f32,
}

enum SampleRateType {
    Hertz,
    Seconds,
}

pub struct SampleRate {
    sr_type: SampleRateType,
    value: u32,
}

impl SampleRate {
    pub fn hertz(value: u32) -> SampleRate {
        SampleRate {sr_type: SampleRateType::Hertz, value: value}
    }

    pub fn seconds(value: u32) -> SampleRate {
        SampleRate {sr_type: SampleRateType::Seconds, value: value}
    }

}

#[derive(Debug)]
struct CustomContentType(String);

impl Clone for CustomContentType {
    fn clone(&self) -> Self {
        CustomContentType(self.0.clone())
    }
}

impl reqwest::header::Header for CustomContentType {
    fn header_name() -> &'static str {
        "Content-Type"
    }

    fn parse_header(raw: &hyper::header::Raw) -> Result<Self, hyper::error::Error> {
        Ok(CustomContentType("".to_string()))
    }

    fn fmt_header(&self, f: &mut hyper::header::Formatter) -> ::std::fmt::Result {
        f.fmt_line(&self.0)
    }
}

pub struct Device {
    version: i32,
    base_path: String,
    auth_key: String,
    http_client: reqwest::Client,
    authenticated: bool,
    auth_token: String,
    server: String,
}

impl Device {
    pub fn upload_data(&mut self, sensor: &str, channel: &str, sample_rate: &SampleRate, data: &Vec<Point>) -> SCResult<()> {
        if !self.authenticated {
            self.authenticate()?;
        }

        let url = "https://".to_string() + &self.server + &self.base_path + "sensors/" + sensor + "/channels/" + 
            channel + "/streams/timeseries/data/?version=1&auth_token=" + &self.auth_token;
        match self.do_upload_data(&url, &sample_rate, &data) {
            Err(Error::ChannelNotFound) => 
                match self.create_sensor(&sensor) {
                    Ok(()) => self.do_upload_data(&url, &sample_rate, &data),
                    Err(error) => Err(error),
                },
            Err(error) => Err(error),
            _ => Ok(()),
        }
    }

    fn do_upload_data(&self, url: &str, sample_rate: &SampleRate, data: &Vec<Point>) -> SCResult<()> {
        let mut body: Vec<u8> = Vec::new();
        let sr: i32 = match sample_rate.sr_type { SampleRateType::Hertz => 1, SampleRateType::Seconds => 0 };
        xdr_codec::pack(&self.version, &mut body).unwrap();
        xdr_codec::pack(&sr, &mut body).unwrap();
        xdr_codec::pack(&sample_rate.value, &mut body).unwrap();
        let dataLen: i32 = data.len() as i32;
        xdr_codec::pack(&dataLen, &mut body).unwrap();
        for p in data.iter() {
            xdr_codec::pack(&p.timestamp, &mut body).unwrap();
            xdr_codec::pack(&p.value, &mut body).unwrap();
        }

        println!("Uploading Data: {}", url);

        let mut req = self.http_client.post(url).header(CustomContentType("application/xdr".to_string())).body(body).
            build().unwrap();
        println!("Headers:\n{}", req.headers());

        match self.http_client.execute(req) {
            Ok(resp) => match resp.status() {
                reqwest::StatusCode::Created => panic!("Upload success"),
                reqwest::StatusCode::NotFound => Err(Error::ChannelNotFound),
                reqwest::StatusCode::Unauthorized => Err(Error::Unauthorized),
                _ => Err(Error::UnknownStatus(resp.status())),
            }
            Err(error) => Err(Error::HTTPError(error)),
        }
    }

    fn create_sensor(&mut self, sensor: &str) -> SCResult<()> {
        let url = "https://".to_string() + &self.server + &self.base_path + "sensors/" + sensor + 
            "/?version=1&auth_token=" + &self.auth_token;
        let mut body: Vec<u8> = Vec::new();
        xdr_codec::pack(&self.version, &mut body).unwrap();
        xdr_codec::pack_string("", Some(50), &mut body).unwrap();
        xdr_codec::pack_string("", Some(50), &mut body).unwrap();
        xdr_codec::pack_string("", Some(1000), &mut body).unwrap();

        match self.http_client.put(&url)
                .header(CustomContentType("application/xdr".to_string()))
                .body(body)
                .send() {
            Ok(mut resp) => if resp.status() == reqwest::StatusCode::Created {
                    Ok(())
                } else  {
                    let resp_content = resp.text().unwrap();
                    println!("{}", &resp_content);
                    Err(Error::InvalidParameters)
                },
            Err(error) => Err(Error::HTTPError(error)),
        }
    }

    fn create_channel(&mut self, sensor: &str, channel: &str) -> SCResult<()> {
        let url = "https://".to_string() + &self.server + &self.base_path + "sensors/" + sensor + "/channels/" + 
            channel + "/?version=1&auth_token=" + &self.auth_token;
        let mut body: Vec<u8> = Vec::new();
        xdr_codec::pack(&self.version, &mut body).unwrap();
        xdr_codec::pack_string("", Some(50), &mut body).unwrap();
        xdr_codec::pack_string("", Some(1000), &mut body).unwrap();

        println!("Creating Channel: {}", url);

        match self.http_client.put(&url)
                .header(CustomContentType("application/xdr".to_string()))
                .body(body)
                .send() {
            Ok(mut resp) => if resp.status() == reqwest::StatusCode::Created {
                    Ok(())
                } else  {
                    let resp_content = resp.text().unwrap();
                    println!("{}", &resp_content);
                    Err(Error::InvalidParameters)
                },
            Err(error) => Err(Error::HTTPError(error)),
        }
    }
            
    fn authenticate(&mut self) -> SCResult<()> {
        let auth_url = "https://sensorcloud.microstrain.com".to_string() + &self.base_path +
            "authenticate/?version=1&key=" + &self.auth_key;
        let mut resp = match self.http_client.get(&auth_url)
                .header(reqwest::header::Accept(vec!(hyper::header::qitem("application/xdr".parse().unwrap()))))
                .send() {
            Ok(response) => if response.status() == reqwest::StatusCode::Ok {
                    response
                } else {
                    return Err(Error::InvalidCredentials);
                },
            Err(error) => return Err(Error::HTTPError(error)),
        };
        let auth_token = xdr_codec::unpack_string(&mut resp, None).unwrap();
        let server = xdr_codec::unpack_string(&mut resp, Some(auth_token.1)).unwrap().0;
        self.auth_token = auth_token.0;
        self.server = server;
        self.authenticated = true;
        Ok(())
    }

    pub fn new(device: &str, key: &str) -> Device {
        Device {
            version: 1,
            base_path: "/SensorCloud/devices/".to_string() + device + "/",
            auth_key: key.to_string(),
            http_client: reqwest::Client::new(),
            authenticated: false,
            auth_token: "".to_string(),
            server: "".to_string(),
        }
    }
}

