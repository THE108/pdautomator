extern crate serde;
extern crate serde_json;
extern crate chrono;
extern crate hyper;
extern crate futures;
extern crate tokio_core;
extern crate hyper_tls;
extern crate failure;

use chrono::{Local, Date};
use tokio_core::reactor::Core;
use futures::{Future, Stream};
use futures::stream::futures_unordered;
use hyper_tls::HttpsConnector;
use hyper::{Method, Request};
use hyper::header::Authorization;

#[derive(Serialize, Deserialize, Debug)]
struct IncidentsResponse {
    incidents: Vec<Incident>,
    limit: u32,
    offset: u32,
    total: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Incident {
    pub id: Option<String>,
    pub incident_number: Option<u32>,
    pub created_on: Option<String>,
    pub status: Option<String>,
    pub service: Option<Service>,
    pub trigger_summary_data: Option<TriggerSummaryData>,
    pub last_status_change_on: Option<String>,
    pub resolved_by_user: Option<User>,
    pub acknowledgers: Option<Vec<Acknowledger>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Service {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TriggerSummaryData {
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Acknowledger {
    pub at: String,
    pub object: Object,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Object {
    pub name: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum IncidentStatus {
    Triggered,
    Acknowledged,
    Resolved,
}

impl IncidentStatus {
    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<IncidentStatus> {
        match s {
            "triggered" => Some(IncidentStatus::Triggered),
            "acknowledged" => Some(IncidentStatus::Acknowledged),
            "resolved" => Some(IncidentStatus::Resolved),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            &IncidentStatus::Triggered => "triggered",
            &IncidentStatus::Acknowledged => "acknowledged",
            &IncidentStatus::Resolved => "resolved",
        }
    }
}

pub struct Client {
    core: Core,
    client: hyper::Client<HttpsConnector<hyper::client::HttpConnector>>,
    token: String,
    org: String,
    timezone: String,
    timezone_short: String,
}

impl Client {
    pub fn new(token: &str, org: &str, timezone: &str, timezone_short: &str) -> Result<Client, failure::Error> {
        let core = Core::new()?;
        let handle = core.handle();
        let tls_connector = HttpsConnector::new(4, &handle)?;
        let client = hyper::Client::configure().connector(tls_connector).build(&handle);

        Ok(Client {
            core,
            client,
            token: token.to_string(),
            org: org.to_string(),
            timezone: timezone.to_string(),
            timezone_short: timezone_short.to_string(),
        })
    }

    fn make_url(&self, since: Option<Date<Local>>, until: Option<Date<Local>>, offset: u32, status: &Option<IncidentStatus>, fields: &Vec<String>) -> Result<hyper::Uri, hyper::error::UriError> {
        let mut params = vec![format!("time_zone={}", self.timezone), format!("offset={}", offset)];

        if let Some(since) = since {
            params.push(format!("since={}T00%3A00%3A00{}", since.format("%Y-%m-%d"), self.timezone_short));
        }

        if let Some(until) = until {
            params.push(format!("until={}T23%3A59%3A59{}", until.format("%Y-%m-%d"), self.timezone_short));
        }

        if let &Some(ref status) = status {
            params.push(format!("status={}", status.as_str()));
        }

        if !fields.is_empty() {
            params.push(format!("fields={}", fields.join(",")));
        }

        let url = format!("https://{}.pagerduty.com/api/v1/incidents?{}", self.org, params.join("&"));

        Ok(url.parse()?)
    }

    fn get(&self, uri: hyper::Uri) -> impl Future<Item = hyper::Chunk, Error = hyper::Error> {
        let mut req = Request::new(Method::Get, uri);
        req.headers_mut().set(Authorization(format!("Token token={}", self.token)));

        Box::new(self.client.request(req).and_then(|res| res.body().concat2()))
    }

    fn parse(&mut self, futs: Vec<impl Future<Item = hyper::Chunk, Error = hyper::Error>>) -> Result<Vec<IncidentsResponse>, failure::Error> {
        let bodies: Vec<hyper::Chunk> = self.core.run(futures_unordered(futs).collect())?;

        let mut responses = Vec::new();
        for body in bodies {
            println!("response: {:?}", ::std::str::from_utf8(&body).expect("error parse response"));

            let r = serde_json::from_slice(&body)?;

            responses.push(r);
        }

        Ok(responses)
    }

    fn parse_incidents(&mut self, futs: Vec<impl Future<Item = hyper::Chunk, Error = hyper::Error>>, incidents: &mut Vec<Incident>) -> Result<(u32, u32), failure::Error> {
        let responses = self.parse(futs)?;

        let mut tupl: (u32, u32) = (0, 0);
        if responses.is_empty() {
            return Ok(tupl);
        }

        if let Some(first) = responses.first() {
            tupl = (first.total, first.limit);
        }

        for mut response in responses {
            if response.incidents.is_empty() {
                continue;
            }

            incidents.append(&mut response.incidents);
        }

        Ok(tupl)
    }

    pub fn get_incidents(&mut self, since: Option<Date<Local>>, until: Option<Date<Local>>, status: Option<IncidentStatus>, fields: Vec<String>) -> Result<Vec<Incident>, failure::Error> {
        let mut offset: u32 = 0;

        let response_future = self.get(self.make_url(since, until, offset, &status, &fields)?);

        let mut result: Vec<Incident> = Vec::new();

        let (total, limit) = self.parse_incidents(vec![response_future], &mut result)?;

        if result.is_empty() {
            return Ok(result);
        }

        let mut futs = Vec::new();
        loop {
            offset += limit;
            if total <= offset {
                break;
            }

            futs.push(self.get(self.make_url(since, until, offset, &status, &fields)?));
        }

        if futs.is_empty() {
            return Ok(result);
        }

        self.parse_incidents(futs, &mut result)?;

        Ok(result)
    }

    pub fn resolve(&mut self, incident_id: &str, requester_id: &str) -> Result<(), failure::Error> {
        let uri = format!("https://{}.pagerduty.com/api/v1/incidents/{}/resolve?requester_id={}", self.org, incident_id, requester_id);
        let mut req = Request::new(Method::Put, uri.parse()?);
        req.headers_mut().set(Authorization(format!("Token token={}", self.token)));

        let fut = self.client.request(req)
            .map(|res| println!("Resolve #{} => {}", incident_id, res.status()));

        self.core.run(fut)?;

        Ok(())
    }
}
