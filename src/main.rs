#![feature(try_trait)]
#![feature(nll)]
#![feature(slice_concat_ext)]
#![feature(match_default_bindings)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

extern crate base64;
extern crate chrono;
#[macro_use]
extern crate hyper;
extern crate itertools;
#[macro_use]
extern crate json;
#[macro_use]
extern crate maplit;
extern crate md5;
#[macro_use]
extern crate p_macro;
extern crate rand;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate uuid;

use std::f64::consts::PI;
use std::slice::SliceConcatExt;
use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};
use std::convert::From;
use std::option::NoneError;
use rand::Rng;
use reqwest::{Client, Request};
use reqwest::header::*;
use itertools::Itertools;
use uuid::Uuid;
use json::JsonValue;
use chrono::{FixedOffset, TimeZone};

#[derive(Default, Debug)]
struct Device {
    imei: String,
    model: String,
    mac: String,
    os_version: String,
    user_agent: String,
    id: String,
    custom_id: String,
}

impl Device {
    pub fn build(&mut self) {
        self.id = format!(
            "{:x}",
            md5::compute(String::new() + &self.imei + &self.model + &self.mac)
        );
        self.custom_id = format!("{:X}", md5::compute(rand::thread_rng().gen::<[u8; 16]>()));
    }
}

#[derive(Default, Debug)]
struct User {
    username: String,
    password: String,
    campus_name: String,
    uid: u32,
    unid: u32,
    token: String,
}

#[derive(Debug, Clone)]
struct FivePoint {
    id: u32,
    pos: GeoPoint,
    name: String,
    fixed: u32,
}

impl FivePoint {
    pub fn to_json(&self, flag: u64) -> JsonValue {
        object! {
            "id" => self.id,
            "flag" => flag,
            "hasReward" => false,
            "isFixed" => self.fixed,
            "isPass" => true,
            "lon" => self.pos.lon,
            "lat" => self.pos.lat,
            "pointName" => self.name.clone(),
            "position" => 999,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Vector {
    x: f64,
    y: f64,
}

impl Vector {
    pub fn distance_to(&self, v: Vector) -> f64 {
        ((v.x - self.x).powf(2.0) + (v.y - self.y).powf(2.0)).sqrt()
    }

    pub fn step_toward(&self, v: Vector, distance: f64) -> Vector {
        let delta = Vector {
            x: v.x - self.x,
            y: v.y - self.y,
        };
        let delta_distance = delta.distance_to(Vector { x: 0.0, y: 0.0 });
        let factor = (distance / delta_distance).min(1.0);

        Vector {
            x: self.x + delta.x * factor,
            y: self.y + delta.y * factor,
        }
    }

    pub fn fuzzle(&self) -> Vector {
        Vector {
            x: rand_near_f64(self.x, FUZZLE_ERR),
            y: rand_near_f64(self.y, FUZZLE_ERR),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GeoPoint {
    lon: f64,
    lat: f64,
}

impl GeoPoint {
    pub fn get_offset_of(&self, origin: Self) -> Vector {
        let dx = self.lon - origin.lon;
        let dy = self.lat - origin.lat;
        let lat_middle = (self.lat + origin.lat) / 2.0;
        let x = (dx * PI / 180.0) * 6367000.0 * (lat_middle * PI / 180.0).cos();
        let y = 6367000.0 * dy * PI / 180.0;
        Vector { x, y }
    }

    pub fn offset(&self, vector: Vector) -> Self {
        let dlat = vector.y * 180.0 / PI / 6367000.0;
        let lat_middle = (self.lat * 2.0 + dlat) / 2.0;
        let dlon = vector.x * 180.0 / PI / (lat_middle * PI / 180.0).cos() / 6367000.0;
        GeoPoint {
            lon: self.lon + dlon,
            lat: self.lat + dlat,
        }
    }
}

#[derive(Debug)]
struct Captcha {
    challenge: String,
    gt: String,
}

#[derive(Debug)]
struct CaptchaResult {
    challenge: String,
    validate: String,
}

#[derive(Debug)]
struct StepRecord {
    id: u32,
    begin: u64,
    end: u64,
    step_count: u32,
    avg_diff: f64,
    max_diff: f64,
    min_diff: f64,
}

impl StepRecord {
    pub fn rand(start_time: u64, end_time: u64) -> Vec<Self> {
        let mut records = Vec::new();
        let mut curr_id = 0;
        let mut curr_time = start_time;

        while curr_time < end_time {
            let prev_time = curr_time;
            curr_time += rand_near_f64(10.0 * 1000.0, SPAMLE_TIME_ERR * 1000.0) as u64;

            records.push(StepRecord {
                id: curr_id,
                begin: prev_time,
                end: curr_time,
                step_count: rand_near(STEP_CNT_PER_10S, STEP_CNT_PER_10S_ERR),
                avg_diff: rand_near_f64(AVG_DIFF, AVG_DIFF_ERR),
                min_diff: rand_near_f64(MIN_DIFF, MIN_DIFF_ERR),
                max_diff: rand_near_f64(MAX_DIFF, MAX_DIFF_ERR),
            });

            curr_id += 1;
        }

        records
    }

    pub fn to_json(&self, flag: u64) -> JsonValue {
        object! {
            "id" => self.id,
            "flag" => flag,
            "beginTime" => self.begin,
            "endTime" => self.end,
            "stepsNum" => self.step_count,
            "minDiff" => self.min_diff,
            "maxDiff" => self.max_diff,
            "avgDiff" => self.avg_diff
        }
    }
}

#[derive(Debug)]
struct SpeedRecord {
    id: u32,
    begin: u64,
    end: u64,
    distance: f64,
}

impl SpeedRecord {
    pub fn rand(start_time: u64, end_time: u64) -> Vec<Self> {
        let mut records = Vec::new();
        let mut curr_id = 0;
        let mut curr_time = start_time;

        while curr_time < end_time {
            let prev_time = curr_time;
            let duration = rand_near_f64(10.0 * 1000.0, SPAMLE_TIME_ERR * 1000.0) as u64;
            curr_time += duration;
            let speed = rand_near_f64(AVG_SPEED, SPEED_ERR);
            let distance = speed * (duration / 1000) as f64;

            records.push(SpeedRecord {
                id: curr_id,
                begin: prev_time,
                end: curr_time,
                distance: distance,
            });

            curr_id += 1
        }

        records
    }

    pub fn to_json(&self, flag: u64) -> JsonValue {
        object! {
            "id" => self.id,
            "beginTime" => self.begin,
            "endTime" => self.end,
            "flag" => flag,
            "distance" => self.distance,
        }
    }
}

#[derive(Debug)]
struct GPSRecord {
    time: u64,
    id: u32,
    speed: f64,
    avg_speed: f64,
    pos: GeoPoint,
    sum_dis: f64,
    sum_time: f64,
}

impl GPSRecord {
    pub fn plan(
        start_time: u64,
        start_pos: GeoPoint,
        distance: f64,
        five_points: &Vec<FivePoint>,
    ) -> Vec<Self> {
        let mut records = Vec::new();
        let mut vectors: Vec<Vector> = five_points
            .iter()
            .map(|p| p.pos.get_offset_of(start_pos))
            .collect();
        let mut curr_id = 0;
        let mut curr_time = start_time;
        let mut curr_pos = Vector { x: 0.0, y: 0.0 };
        let mut sum_time = 0.0;
        let mut sum_dis = 0.0;

        while sum_dis < distance || vectors.len() > 0 {
            let speed = rand_near_f64(AVG_SPEED, SPEED_ERR);
            let duration = rand_near_f64(SPAMLE_TIME * 1000.0, SPAMLE_TIME_ERR * 1000.0);
            let distance = speed * duration / 1000.0;

            if vectors.len() > 0 {
                // Towards point
                let target = vectors.last().unwrap();
                curr_pos = curr_pos.step_toward(*target, distance).fuzzle();
                if curr_pos.distance_to(*target) < 5.0 {
                    vectors.pop();
                }
            } else {
                // Towards north
                curr_pos = curr_pos
                    .step_toward(Vector { x: 0.0, y: 10000.0 }, distance)
                    .fuzzle();
            }

            let speed_weird = (50.0 * sum_time) / (3.0 * sum_dis);
            let avg_speed_weird = rand_near_f64(speed_weird, 0.2);
            records.push(GPSRecord {
                id: curr_id,
                time: curr_time,
                speed: speed_weird,
                avg_speed: avg_speed_weird,
                pos: start_pos.offset(curr_pos),
                sum_dis: sum_dis,
                sum_time: sum_time,
            });

            curr_id += 1;
            curr_time += duration.round() as u64;
            sum_dis += distance;
            sum_time += duration / 1000.0;
        }

        records
    }

    pub fn to_json(&self, flag: u64) -> JsonValue {
        let time_zone = FixedOffset::east(8 * 3600);
        let time_format = time_zone
            .timestamp(self.time as i64 / 1000, 0)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        object! {
            "id" => self.id,
            "flag" => flag,
            "lat" => self.pos.lat,
            "lng" => self.pos.lon,
            "totalDis" => self.sum_dis / 1000.0,
            "totalTime" => self.sum_time.round() as u32,
            "speed" => self.speed,
            "avgSpeed" => self.avg_speed,
            "gainTime" => time_format,
            "locType" => 61,
            "radius" => 180,
            "type" => 1,
        }
    }
}

#[derive(Debug)]
struct RunRecord {
    uuid: String,
    start_pos: GeoPoint,
    sel_distance: u32,
    distance: f64,
    five_points: Vec<FivePoint>,
    start_time: u64,
    end_time: u64,
    gps_records: Vec<GPSRecord>,
    step_records: Vec<StepRecord>,
    speed_records: Vec<SpeedRecord>,
}

impl RunRecord {
    pub fn plan(
        uuid: &String,
        start_pos: GeoPoint,
        sel_distance: u32,
        distance: f64,
        five_points: &Vec<FivePoint>,
        start_time: u64,
    ) -> Self {
        let gps_records = GPSRecord::plan(start_time, start_pos, distance, five_points);
        let end_time = gps_records.last().unwrap().time + 5000;
        let step_records = StepRecord::rand(start_time, end_time);
        let speed_records = SpeedRecord::rand(start_time, end_time);

        RunRecord {
            uuid: uuid.clone(),
            start_pos,
            sel_distance,
            distance,
            five_points: five_points.to_vec(),
            start_time,
            end_time,
            gps_records,
            step_records,
            speed_records,
        }
    }

    pub fn to_json(&self, flag: u64, uid: u32, unid: u32) -> JsonValue {
        let all_loc_json = JsonValue::Array(
            self.gps_records.iter().map(|r| r.to_json(flag)).collect(),
        ).to_string();

        let five_point_json = JsonValue::Array(
            self.five_points.iter().map(|p| p.to_json(flag)).collect(),
        ).to_string();

        let speed_records =
            JsonValue::Array(self.speed_records.iter().map(|r| r.to_json(flag)).collect());
        let step_records =
            JsonValue::Array(self.step_records.iter().map(|r| r.to_json(flag)).collect());

        let mut json = object! {
            "avgStepFreq" => rand_near(STEP_CNT_PER_MIN, STEP_CNT_PER_MIN_ERR),
            "calorie" => rand_near(CALORIE, CALORIE_ERR),
            "complete" => true,
            "getPrize" => false,
            "selDistance" => self.sel_distance,
            "selectedUnid" => unid,
            "speed" => rand_near_f64(AVG_SPEED, SPEED_ERR),
            "sportType" => 1,
            "startTime" => self.start_time,
            "status" => 0,
            "stopTime" => self.end_time,
            "totalDis" => self.distance / 1000.0,
            "totalSteps" => self.step_records.iter().fold(0, |sum, record| sum + record.step_count),
            "totalTime" => ((self.end_time - self.start_time) / 1000) as u32,
            "uid" => uid,
            "unCompleteReason" => 0,
            "uuid" => self.uuid.clone(),
        };

        let mut sign_param = BTreeMap::new();

        {
            for (k, v) in json.entries() {
                sign_param.insert(k.to_string(), v.to_string());
            }
        }

        let signature = compute_sign(&sign_param, MD5_SIGN_SALT_RUN);

        p!(json);
        p!(sign_param);
        p!(signature);

        let json_extend = object! {
            "allLocJson" => object! {
                "allLocJson" => all_loc_json,
            },
            "fivePointJson" => object!{
                "fivePointJson" => five_point_json,
            },
            "speedPerTenSec" => speed_records,
            "stepPerTenSec" => step_records,
            "isUpload" => false,
            "more" => true,
            "unid" => unid,
            "signature" => signature,
        };

        match (&mut json, &json_extend) {
            (JsonValue::Object(obj), JsonValue::Object(obj_extend)) => {
                for (k, v) in obj_extend.iter() {
                    obj.insert(k, v.clone());
                }
            }
            _ => unreachable!(),
        }

        p!(json);
        p!(json_extend);

        json
    }
}

const AVG_SPEED: f64 = 3.0;
const SPEED_ERR: f64 = 2.0;
const SPAMLE_TIME: f64 = 6.0;
const SPAMLE_TIME_ERR: f64 = 1.0;
const CALORIE: u32 = 300;
const CALORIE_ERR: u32 = 100;
const STEP_CNT_PER_10S: u32 = 15;
const STEP_CNT_PER_10S_ERR: u32 = 7;
const STEP_CNT_PER_MIN: u32 = 60;
const STEP_CNT_PER_MIN_ERR: u32 = 10;
const AVG_DIFF: f64 = 24.0;
const AVG_DIFF_ERR: f64 = 10.0;
const MIN_DIFF: f64 = 7.0;
const MIN_DIFF_ERR: f64 = 3.0;
const MAX_DIFF: f64 = 40.0;
const MAX_DIFF_ERR: f64 = 15.0;
const FUZZLE_ERR: f64 = 2.0;

const APP_VERSION: &'static str = "2.0.0";
const OS_TYPE: &'static str = "0";
const MD5_KEY: &'static str = "05df15504f394eab8dd3ab8180006a83";
const MD5_SIGN_SALT: &'static str = "&wh2016_swcampus";
const MD5_SIGN_SALT_RUN: &'static str = "&ODJw#h03b_0EaV";

#[derive(Debug)]
struct App {
    device: Device,
    user: User,
    client: Client,
}

fn compute_sign(map: &BTreeMap<String, String>, salt: &str) -> String {
    let str = map.iter().map(|(k, v)| format!("{}={}", k, v)).join("&");
    format!("{:x}", md5::compute(str.clone() + salt))
}

fn validate(text: &str) -> Result<JsonValue, Error> {
    let json = json::parse(&text)?;
    if json["error"] != 10000 {
        return Err(Error::Api(json["message"].as_str()?.to_string()));
    }
    Ok(json)
}

fn rand_near(base: u32, err: u32) -> u32 {
    base + (err * (rand::thread_rng().next_f64() * 2.0 - 1.0) as u32)
}

fn rand_near_f64(base: f64, err: f64) -> f64 {
    base + err * (rand::thread_rng().next_f64() * 2.0 - 1.0)
}

impl App {
    pub fn new(mut device: Device, user: User) -> App {
        device.build();
        App {
            device,
            user,
            // client: Client::new(),
            client: Client::builder()
                .proxy(reqwest::Proxy::https("http://127.0.0.1:8888").unwrap())
                .proxy(reqwest::Proxy::http("http://127.0.0.1:8888").unwrap())
                .build()
                .unwrap(),
        }
    }

    fn headers_user_agent(&mut self) -> Headers {
        let mut headers = Headers::new();
        headers.set_raw("User-Agent", self.device.user_agent.clone());
        headers
    }

    fn headers(&mut self) -> Headers {
        let mut headers = Headers::new();
        headers.set_raw("Accept", "application/json");
        headers.set_raw("User-Agent", self.device.user_agent.clone());
        headers.set_raw("Content-Type", "application/json;charset=UTF-8");
        headers.set_raw("appVersion", APP_VERSION);
        headers.set_raw("CustomDeviceId", self.device.custom_id.clone());
        headers.set_raw("DeviceId", self.device.id.clone());
        headers.set_raw("osType", OS_TYPE);
        headers.set_raw("osVersion", self.device.os_version.clone());

        if self.user.token != "" {
            let now = SystemTime::now();
            let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
            let time_stamp = since_the_epoch.as_secs() * 1000
                + since_the_epoch.subsec_nanos() as u64 / 1_000_000;

            let sign_param = btreemap! {
                "uid".to_string() => self.user.uid.to_string(),
                "token".to_string() => self.user.token.clone(),
                "timeStamp".to_string() => time_stamp.to_string(),
            };
            let sign = compute_sign(&sign_param, MD5_SIGN_SALT);

            headers.set_raw("uid", self.user.uid.to_string());
            headers.set_raw("token", self.user.token.clone());
            headers.set_raw("timeStamp", time_stamp.to_string());
            headers.set_raw("tokenSign", sign);
        }

        headers
    }

    pub fn login(&mut self) -> Result<(), Error> {
        let auth_key = base64::encode(&format!("{}:{}", self.user.username, self.user.password));
        let auth_str = format!("Basic {}", auth_key);

        let mut headers = self.headers();
        headers.set_raw("Authorization", auth_str);

        let json = object!{
            "app_version" => APP_VERSION,
            "channel" => "",
            "device_id" => self.device.id.clone(),
            "device_model" => self.device.model.clone(),
            "imei" => self.device.imei.clone(),
            "loginType" => 1,
            "mac_address" => self.device.mac.clone(),
            "os_type" => 0,
            "os_version" => self.device.os_version.clone(),
        };

        let res = self.client
            .post("https://gxapp.iydsj.com/api/v23/login")
            .headers(headers)
            .body(json.to_string())
            .send()?
            .text()?;

        let res = json::parse(&res)?;

        if res["error"] != 10000 {
            return Err(Error::Api(res["message"].as_str()?.to_string()));
        }

        let data = &res["data"];

        self.user.token = data["token"].as_str()?.to_string();
        self.user.uid = data["uid"].as_u32()?;
        self.user.unid = data["unid"].as_u32()?;
        self.user.campus_name = data["campusName"].as_str()?.to_string();

        Ok(())
    }

    pub fn fetch_points(
        &mut self,
        start_pos: GeoPoint,
        distance: u32,
    ) -> Result<Vec<FivePoint>, Error> {
        let sign_str = format!("http://gxapp.iydsj.com/api/v18/get/1/distance/{}", distance);
        let sign = format!("{:X}", md5::compute(sign_str + MD5_KEY));

        let json = object!{
            "latitude" => start_pos.lat,
            "longitude" => start_pos.lon,
            "selectedUnid" => self.user.unid,
            "sign" => sign,
        };

        let res = self.client
            .post("https://gxapp.iydsj.com/api/v18/get/1/distance/2000")
            .headers(self.headers())
            .body(json.to_string())
            .send()?
            .text()?;

        let res = validate(&res)?;

        let data = &res["data"]["pointsResModels"];

        data.members()
            .enumerate()
            .map(|(i, obj)| {
                Ok(FivePoint {
                    id: i as u32,
                    name: obj["pointName"].as_str()?.to_string(),
                    fixed: obj["isFixed"].as_u32()?,
                    pos: GeoPoint {
                        lon: obj["lon"].as_f64()?,
                        lat: obj["lat"].as_f64()?,
                    },
                })
            })
            .collect()
    }

    pub fn start_validate(&mut self, uuid: &String) -> Result<Captcha, Error> {
        let res = self.client
            .get("https://gxapp.iydsj.com/api/v20/security/geepreprocess")
            .headers(self.headers_user_agent())
            .query(&[
                ("osType", OS_TYPE),
                ("uid", &self.user.uid.to_string()),
                ("uuid", &uuid),
            ])
            .send()?
            .text()?;

        let res = validate(&res)?;

        let data = &res["data"];

        Ok(Captcha {
            challenge: data["challenge"].as_str()?.to_string(),
            gt: data["gt"].as_str()?.to_string(),
        })
    }

    pub fn anti_test(&mut self, captcha: &Captcha, apikey: String) -> Result<CaptchaResult, Error> {
        let res = self.client
            .get("http://jiyan.25531.com/api/create")
            .query(&[
                ("appkey", apikey),
                ("gt", captcha.gt.clone()),
                ("challenge", captcha.challenge.clone()),
                ("referer", "".to_string()),
                ("model", 3.to_string()),
            ])
            .send()?
            .text()?;

        p!(res);

        let res = json::parse(&res)?;

        if res["code"] != 10000 {
            return Err(Error::Api(res["data"].as_str()?.to_string()));
        }

        let data = &res["data"];

        Ok(CaptchaResult {
            challenge: data["challenge"].as_str()?.to_string(),
            validate: data["validate"].as_str()?.to_string(),
        })
    }

    pub fn post_validate(&mut self, uuid: &String, captcha: &CaptchaResult) -> Result<(), Error> {
        let params = hashmap! {
            "uid" => self.user.uid.to_string(),
            "osType" => OS_TYPE.to_string(),
            "uuid" => uuid.clone(),
            "geetest_challenge" => captcha.challenge.clone(),
            "geetest_seccode" => captcha.validate.clone(),
            "geetest_validate" => captcha.validate.clone(),
        };

        let res = self.client
            .post("https://gxapp.iydsj.com/api/v20/security/geevalidate")
            .headers(self.headers_user_agent())
            .form(&params)
            .send()?
            .text()?;

        let res = validate(&res)?;

        Ok(())
    }

    pub fn post_record(&mut self, record: &RunRecord) -> Result<(), Error> {
        let data = "";

        let res = self.client
            .post("https://gxapp.iydsj.com/api/v22/runnings/save/record")
            .headers(self.headers())
            .body(data.to_string())
            .send()?
            .text()?;

        let res = validate(&res)?;

        Ok(())
    }
}

#[derive(Debug)]
enum Error {
    IO(reqwest::Error),
    Parse(json::Error),
    Api(String),
}

impl From<NoneError> for Error {
    fn from(er_ror: NoneError) -> Self {
        Error::Api("json incomplete".to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::IO(error)
    }
}

impl From<json::Error> for Error {
    fn from(error: json::Error) -> Self {
        Error::Parse(error)
    }
}

fn main() {
    let API_KEY = "78c7d1e23f8a0d453338d2f9cdabbbf7".to_string();

    let device = Device {
        imei: "".into(),
        model: "Xiaomi MI 4LTE".into(),
        mac: "58:44:98:21:59:7".into(),
        os_version: "6.0.1".into(),
        user_agent: "Dalvik/2.1.0 (Linux; U; Android 6.0.1; MI 4LTE Build/MMB29M)".into(),
        ..Default::default()
    };

    let user = User {
        username: "2017040505323".into(),
        password: "505323".into(),
        ..Default::default()
    };

    let start_pos = GeoPoint {
        lat: 23.169042,
        lon: 113.044233,
    };

    let sel_distance = 2000;
    let start_time = 1521520225299;

    let flag = start_time - rand_near(30 * 60 * 1000, 5 * 60 * 1000) as u64;

    let mut app = App::new(device, user);

    app.login().unwrap();

    let five_points = app.fetch_points(start_pos, sel_distance).unwrap();

    let uuid = Uuid::new_v4().hyphenated().to_string();

    let record = RunRecord::plan(
        &uuid,
        start_pos,
        sel_distance,
        sel_distance as f64 + 100.0,
        &five_points,
        start_time,
    );

    p!(record
        .step_records
        .iter()
        .foreach(|r| println!("{}", r.to_json(flag).to_string())));
    p!(record
        .speed_records
        .iter()
        .foreach(|r| println!("{}", r.to_json(flag).to_string())));
    p!(record
        .gps_records
        .iter()
        .foreach(|r| println!("{}", r.to_json(flag).to_string())));
    p!(record.to_json(flag, 123, 8756).to_string());

    // let captcha = app.start_validate(&uuid).unwrap();

    // let captcha_result = app.anti_test(&captcha, API_KEY).unwrap();
    // let captcha_result = CaptchaResult {
    //     challenge: "a".to_string(),
    //     validate: "v".to_string(),
    // };

    // p!(five_points);
    // p!(captcha);
    // p!(captcha_result);

    // app.post_validate(&uuid, &captcha_result).unwrap();
}
