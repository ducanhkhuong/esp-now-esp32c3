
### Rust Language : esp-now
## 1. build + flash master
```
cd master
```
```
cargo build --release
```
```
cargo espflash flash --monitor 
```
## 2. build + flash slave
```
cd slave
```
```
cargo build --release
```
```
cargo espflash flash --monitor 
```



