# pdautomator
Pagerduty simple task automation tool.

## What is it for?
`pdautomator` connects pagerduty alerts with user defined actions.

## Configuration

### User defined actions configuration
You can map an alert to an action by specifying a mapping in the `actions` block in config file:

```toml
[[actions]]
alert = '''service\.(?P<prefix>[a-z]{2,4})\.(?P<ip1>\d{1,3})_(?P<ip2>\d{1,3})_(?P<ip3>\d{1,3})_(?P<ip4>\d{1,3})\.error'''
cmd = "/home/user/dosomething.sh $prefix $ip1.$ip2.$ip3.$ip4"
```

Here every opened alert which matches regexp provided in `alert` field will trigger an execution of user defined command provided in `cmd`.

It's possible to substitude some placeholders with matched groups from the alert regexp, i.e. an alert `service.ds1.192_168_0_1.error` will trigger execution of the following command: `/home/user/dosomething.sh ds1 192.168.0.1`.

Also, you can specify a pause (in seconds) between command execution specified in the same action block.

```toml
[[actions]]
alert = '''service\.(?P<prefix>[a-z]{2,4})\.(?P<ip1>\d{1,3})_(?P<ip2>\d{1,3})_(?P<ip3>\d{1,3})_(?P<ip4>\d{1,3})\.error'''
cmd = "/home/user/dosomething.sh $prefix $ip1.$ip2.$ip3.$ip4"
pause_sec = 60
```

So, if two alerts which match with `alert` regexp were fired simultaniously (actually fetched during the same `pdautomator` run) the second alert will wait for 60 seconds before it will trigger the command execution.

Alerts could be resolved by `pdautomator` after user defined command has been executed.

```
[[actions]]
alert = '''service\.(?P<prefix>[a-z]{2,4})\.(?P<ip1>\d{1,3})_(?P<ip2>\d{1,3})_(?P<ip3>\d{1,3})_(?P<ip4>\d{1,3})\.error'''
cmd = "/home/user/dosomething.sh $prefix $ip1.$ip2.$ip3.$ip4"
resolve = true
resolve_check = '''Status: OK\s+'''
```

Here every opened alert which matches regexp provided in `alert` field will be resolved if the command's output (`stdout`) matches with `resolve_check` regexp. Note, alerts will be resolved using `requester_id` (user id) from `pagerduty` section.

### Pagerduty configuration

Here is the pagerduty related config section:
```toml
[pagerduty]
org = "org" # https://{org subdomain}.pagerduty.com/
token = "token" # api security token
timezone = "Singapore"
timezone_short = "SGT"
since_days = 1 # number of days to fetch alerts from
requester_id = "ABC1234" # pagerduty user id
```

## Installation

You need at least [rust](https://www.rust-lang.org/en-US/install.html) 1.26 to compile `pdautomator`.

```bash
$ cargo build --release
```

## Lisence

MIT
