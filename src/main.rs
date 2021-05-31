// WARNING!!! Ugly code
extern crate rusoto_core;
extern crate rusoto_s3;
#[macro_use]
extern crate log;

use futures::future::{BoxFuture, FutureExt};
use rusoto_core::Region;
use rusoto_s3::{ListObjectsV2Request, S3Client, S3};
use serde_derive::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::Infallible;
use warp::{Filter};

#[derive(Debug)]
struct LastOne {
    size: i64,
    date: i64,
    date_str: String,
    key: String,
}
impl PartialOrd for LastOne {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.date.partial_cmp(&other.date)
    }
}
impl Ord for LastOne {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date.cmp(&other.date)
    }
}
impl PartialEq for LastOne {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date
    }
}
impl Eq for LastOne {}

#[derive(Debug, Deserialize, Serialize)]
struct PromRequest {
    module: String,
    target: String,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let hello = warp::any().and_then(got).with(warp::log("requests"));
    let prom = warp::get().and(warp::query::<PromRequest>()).and_then(got2).with(warp::log("requests"));
    println!("waiting on :8080");
    warp::serve(prom.or(hello)).run(([0, 0, 0, 0], 8080)).await;
}

fn get_part(
    client: S3Client,
    mut bucket_request: ListObjectsV2Request,
    cont_token: Option<String>,
    last: Option<LastOne>,
) -> BoxFuture<'static, (Option<String>, Option<LastOne>)> {
    async move {
        bucket_request.continuation_token = cont_token;
        match client.list_objects_v2(bucket_request.clone()).await {
            Err(e) => {
                error!("list_object: {:#?}", e);
                (None, last)
            }
            Ok(res) if res.contents == None => {
                error!("list_object: None");
                (None, last)
            }
            Ok(res) => {
                error!("list_object: {:#?}", res);
                let latest = res
                    .contents
                    .unwrap()
                    .into_iter()
                    .map(|x| LastOne {
                        date_str: x.last_modified.clone().unwrap(),
                        date: chrono::DateTime::parse_from_rfc3339(&x.last_modified.unwrap())
                            .unwrap()
                            .timestamp(),
                        size: x.size.unwrap(),
                        key: x.key.unwrap(),
                    })
                    .max()
                    .max(last);
                warn!("Latest: {:?}", latest);

                match res.next_continuation_token {
                    None => (None, latest),
                    Some(x) => get_part(client, bucket_request, Some(x), latest).await,
                }
            }
        }
    }
    .boxed()
}

async fn got2(param: PromRequest) -> Result<impl warp::Reply, Infallible> {
    info!("Got params: {:?}", param);
    let target: HashMap<&str, &str> = param.target
        .split(',')
        .map(|s| s.split_at(s.find('=').unwrap()))
        .map(|(k, v)| (k, &v[1..]))
        .collect();


    let region = Region::EuWest1;
    let client = S3Client::new(region);
    let bucket = target.get("bucket").unwrap().to_owned();
    let prefix = target.get("prefix").unwrap().to_owned();

    let bucket_request = ListObjectsV2Request {
        bucket: bucket.to_owned(),
        prefix: Some(prefix.to_owned()),
        // max_keys: Some(3), // DEBUG
        continuation_token: None,
        ..Default::default()
    };

    let (_, ret) = get_part(client, bucket_request, None, None).await;

    match ret {
        Some(ret) => {
            let tmplt = format!(
                "{{bucket=\"{}\", prefix=\"{}\", atype=\"tg\", filename=\"{}\"}}",
                bucket, prefix, ret.key
            );
            Ok(format!(
                "s3exp_date{} {}\ns3exp_success{} 1\ns3exp_size{} {}\n",
                tmplt, ret.date, tmplt, tmplt, ret.size
            ))
        }
        None => {
            error!("list_objects_v2 ans is None");
            Ok(format!(
                "s3exp_success 0\ns3exp_err{{type=\"{}\"}} 1\n",
                "none_answer"
            ))
        }
    }
}
async fn got() -> Result<impl warp::Reply, Infallible> {
    let region = Region::EuWest1;
    let client = S3Client::new(region);
    let bucket = "cms-msl-db-backups";
    let prefix = "mysql/k8production";

    let bucket_request = ListObjectsV2Request {
        bucket: bucket.to_owned(),
        prefix: Some(prefix.to_owned()),
        continuation_token: None,
        ..Default::default()
    };

    let (_, ret) = get_part(client, bucket_request, None, None).await;

    match ret {
        Some(ret) => {
            let tmplt = format!(
                "{{bucket=\"{}\", prefix=\"{}\", atype=\"tg\", filename=\"{}\"}}",
                bucket, prefix, ret.key
            );
            Ok(format!(
                "s3exp_date{} {}\ns3exp_success{} 1\ns3exp_size{} {}\n",
                tmplt, ret.date, tmplt, tmplt, ret.size
            ))
        }
        None => {
            error!("list_objects_v2 ans is None");
            Ok(format!(
                "s3exp_success 0\ns3exp_err{{type=\"{}\"}} 1\n",
                "none_answer"
            ))
        }
    }
    //Ok(format!("{:#?}", ret))
}
