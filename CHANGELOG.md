# Changelog

## 0.3.0

* [Breaking change] upgrade to tokio 1.0

## 0.2.0

* [Breaking change] Changed the type of functions payload arguments to handle
  any JSON-serializable value (msgpack is also working, but we currently use
  `serde_json::Value`, which has its limits)
* [Breaking change] upgrade to tokio 0.3
* Enable additional HTTP header for WebSocket transport
* Implemented Client WAMP-level Authentication [advanced profile feature]

## 0.1.1

* Migrated to GitHub
