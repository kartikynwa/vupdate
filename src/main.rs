/*
* Motivation: Void Linux packages are built using `templates` similar to Arch
* Linux. The templates are hosted in the git repository at:
*   - https://github.com/void-linux/void-packages
*
* To update a package to a new version, the `template` file has to be updated
* for the corresponding package and a pull request has to be filed.
*
* Each template lists a maintaianer who is supposed to keep the package working
* and up to date as much as possible. For convenience, this text file is
* updated daily with names of packages for which updates are available
* upstream:
*   - https://alpha.de.repo.voidlinux.org/void-updates/void-updates.txt
* Package updates are listed in the following format:
*
* ```
* python-mock  3.0.5 -> 4.0.3    https://github.com/testing-cabal/mock
* ```
*
* Additionally, available updates for a specific maintainer are also published.
* For example, updates for me are posted at:
*   - https://alpha.de.repo.voidlinux.org/void-updates/void-updates/updates_kartik.ynwa@gmail.com.txt
*
* But sometimes users want to update packages for which they are not the
* maintainers. This program is meant for this user. It fetches the updates.txt
* files, parses them and prints a list of packages for which updates are available
* upstream. This includes:
* - Packages for which the user is the maintainer.
* - Packages which are installed on the system.
*
* TODO:
*   - Don't hardcode user email.
*   - Support an ignore file to prevent clutter.
*/

use colored::Colorize;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::process::Command;

// Will be using this to construct URLs for making HTTP requests
static VOID_URL: &str = "https://alpha.de.repo.voidlinux.org/void-updates/void-updates";
static EMAIL: &str = "kartik.ynwa@gmail.com";

// Data type for storing package update information
struct PackageUpdate {
    current_version: String,
    new_version: String,
}

// Type alias for storing a directory of packages and their update information
struct UpdateMap(HashMap<String, PackageUpdate>);

impl UpdateMap {
    fn new() -> UpdateMap {
        UpdateMap(HashMap::new())
    }
}

impl std::fmt::Display for UpdateMap {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut keys: Vec<&String> = self.0.keys().collect();
        keys.sort();
        for key in keys {
            let update = self.0.get(key).unwrap();
            writeln!(
                f,
                "{}\t{} -> {}",
                key, update.current_version, update.new_version
            )?;
        }
        Ok(())
    }
}

// Function to make an HTTP request and return the body as a String
async fn get_http_response(url: &str) -> Result<String, reqwest::Error> {
    let response = reqwest::get(url).await?;
    let text = response.text().await.unwrap();
    Ok(text)
}

// Get a list of installed packages by running the command `xbps-query -l` and
// parsing the output.
fn get_installed_packages() -> HashSet<String> {
    let xq_output = Command::new("xbps-query")
        .arg("-m")
        .output()
        .expect("Could not run xbps-query");
    let xq_stdout = String::from_utf8(xq_output.stdout).unwrap();

    let re = Regex::new(r"(\S+)-\S+?").unwrap();

    xq_stdout
        .lines()
        .filter_map(|l| re.captures(l))
        .map(|c| String::from(&c[1]))
        .collect()
}

// Get the names of packages for which updates are available and for which I am
// the maintainer.
async fn get_maintainer_updates() -> Result<UpdateMap, reqwest::Error> {
    let url = format!("{}/updates_{}.txt", VOID_URL, EMAIL);
    let body = get_http_response(&url).await?;
    Ok(response_to_hashmap(&body))
}

// Get the names of installed packages for updates are available.
async fn get_all_updates() -> Result<UpdateMap, reqwest::Error> {
    let url = format!("{}{}", VOID_URL, ".txt");

    let body = get_http_response(&url).await?;
    Ok(response_to_hashmap(&body))
}

// Parse the response body from updates.txt files into an UpdateMap
fn response_to_hashmap(body: &str) -> UpdateMap {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(\S+)\s+(\S+)\s+->\s+(\S+)").unwrap();
    }

    let mut pkg_updates = UpdateMap::new();

    for cap in body.lines().filter_map(|l| RE.captures(l)) {
        let pkg_name = String::from(&cap[1]);
        let pkg_update = PackageUpdate {
            current_version: String::from(&cap[2]),
            new_version: String::from(&cap[3]),
        };
        let mut insert = true;
        if let Some(existing_pkg_update) = pkg_updates.0.get(&pkg_name) {
            if pkg_update.new_version <= existing_pkg_update.new_version {
                insert = false;
            }
        }
        if insert {
            pkg_updates.0.insert(pkg_name, pkg_update);
        }
    }
    pkg_updates
}

#[tokio::main]
async fn main() {
    let (maintainer_updates_result, installed_updates_result) =
        tokio::join!(get_maintainer_updates(), get_all_updates());

    let maintainer_updates = match maintainer_updates_result {
        Ok(updates) => updates,
        _ => {
            let error_msg = format!("Could not fetch updates_{}.txt", EMAIL);
            println!("{}", &error_msg.red());
            UpdateMap::new()
        }
    };

    let mut installed_updates = match installed_updates_result {
        Ok(updates) => updates,
        _ => {
            println!("{}", &"Could not fetch void-updates.txt".red());
            UpdateMap::new()
        }
    };

    // Only keep updates for packages that are: a) Installed, b) Not being maintained by me
    let installed_pkgs = get_installed_packages();
    installed_updates
        .0
        .retain(|k, _| installed_pkgs.contains(k) && !maintainer_updates.0.contains_key(k));

    // Print packages for which I am the maintainer
    if !maintainer_updates.0.is_empty() {
        println!("{}", &"Maintainer updates:".bold().blue().underline());
        println!("{}", &maintainer_updates);
    }

    // Print packages which are currently installed
    if !installed_updates.0.is_empty() {
        println!(
            "{}",
            &"Updates for installed packages:".bold().blue().underline()
        );
        print!("{}", &installed_updates);
    }
}
