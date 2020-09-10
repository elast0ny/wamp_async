# Changelog

## 0.2.0

* [Breaking change] Changed the type of functions payload arguments to handle
  any JSON-serializable value (msgpack is also working, but we currently use
  `serde_json::Value`, which has its limits)

## 0.1.1

* Migrated to GitHub
