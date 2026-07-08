app vietlott:
_sẽ cào dữ liệu từ đường dẫn này https://vietlott.vn/vi/trung-thuong/ket-qua-trung-thuong/645?id=01532&nocatche=1
_lưu lại thành các file 00001.txt -> 01532.txt vào vietlott-raw
_kiểm tra các file thiếu theo id cho trước
_parser dữ liệu vào file vietlott_6-45.txt

build app:

cargo build --release

run app:

cargo run --bin vietlott

đóng gói app cho linux debian:

cargo deb

chú ý: thiếu thư viện gì thì lên gemini search rồi cài vô máy, ngoài ra nếu bạn dùng vpn hoặc ip ko thuộc việt nam thì web vietlott sẽ hiện captcha vào app hoàn toàn vô dụng...

