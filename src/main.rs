const REST_URL: &str = "https://localhost/rest/";
// TODO: Stop pasting these "secrets" into the code. Fine for now as its only local.
const USER_ID: &str = "54828a041facb13484c2014d7d3cf8fa";
const USER_TOKEN: &str = "ZBT0E4UaVk4xV4bHxTwxLETCaXEr7QO5";

/// <http://localhost/seitentypen/blog/eintrag-1>
const TEST_SITE: &str = "62";

/// <http://localhost/admin.php?site=materialkit&lang=de&id=56>
const SITE_URL: &str = "materialkit/de/";

pub mod communicator;

use communicator::Communicator;
use serde_json::{Map, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let communicator = Communicator::new(
        REST_URL.to_string(),
        SITE_URL.to_string(),
        USER_ID.to_string(),
        USER_TOKEN.to_string(),
    )?;

    let mut result = communicator.get_page(TEST_SITE)?.json::<Value>()?;

    result["extra"]["_contents"]["center"][0]["content1"] = "<p>Success!</p>".into();

    let mut new_extra = Map::new();
    new_extra.insert("extra".into(), result["extra"].take());

    let _ = communicator.update_extra(TEST_SITE, &new_extra.into())?;

    let result = communicator.get_page(TEST_SITE)?;
    println!("{}", result.json::<Value>()?);
    println!();

    Ok(())
}
