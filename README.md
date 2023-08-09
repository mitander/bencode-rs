# bencode-rs
A bencode parsing library using [nom](https://github.com/rust-bakery/nom)


# Usage
```rust
// Parse read bytes to Value::Dictionary
let data = Value::parse(include_bytes!("../test-assets/test.torrent")).unwrap();
let v = data.first().unwrap();

// Index dict values
if let Value::Dictionary(dict) = v {
    let info = dict.get(b"info".as_slice()).unwrap();
    if let Value::Dictionary(info) = info {
        let v = info.get(b"length".as_slice()).unwrap();
        let v = info.get(b"name".as_slice()).unwrap();
    }

    let announce = dict.get(b"announce".as_slice()).unwrap();
    if let Value::Bytes(announce) = *announce {
        let str = std::str::from_utf8(announce).unwrap();
    }

    let created_by = dict.get(b"created by".as_slice()).unwrap();
    if let Value::Bytes(created_by) = *created_by {
        let str = std::str::from_utf8(created_by).unwrap();
    }
}

```
