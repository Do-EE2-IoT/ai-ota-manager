# Hướng Dẫn OTA AI Cho Người Mới

Tài liệu này hướng dẫn cách thêm HC mới, sửa config, thêm model custom và push lên GitHub để HC tự OTA.

Người thao tác **không cần tự tính hoặc tự sửa SHA256 bằng tay**. Sau khi sửa file, chỉ cần chạy tool refresh SHA ở bước cuối.

## Luồng Production Hiện Tại

HC chạy cron mỗi 5 phút và đọc dữ liệu từ GitHub raw:

```text
https://raw.githubusercontent.com/Do-EE2-IoT/ai-ota-manager/main
```

Script OTA trên HC nằm tại:

```text
/etc/smarthome/ai-ota-manager/scripts/check-model-update.sh
```

Log OTA trên HC nằm tại:

```text
/var/log/ai-ota-manager.log
```

HC quyết định update bằng version trong:

```text
/etc/smarthome/ota_ai_info.json
```

Nguyên tắc quan trọng:

- Remote version trong `manifest.json` phải lớn hơn local version thì HC mới update.
- SHA256 dùng để verify file hợp lệ, nhưng người thao tác không cần sửa tay.
- Model custom không ghi đè model gốc.
- Nếu update fail, script rollback config về model gốc.

## Cấu Trúc Quan Trọng

```text
model/
├── detect_model_v2_custom.lum
└── verify_model_v2_custom.lum

hc/
└── 24-95-07-e0-81-96/
    ├── manifest.json
    └── config.json
```

## Model Lưu Ở Đâu

Model custom lưu trong repo tại:

```text
model/
```

Ví dụ:

```text
model/detect_model_v2_custom.lum
model/verify_model_v2_custom.lum
```

Sau khi OTA, HC tải model về:

```text
/etc/smarthome/detect_model_v2_custom.lum
/etc/smarthome/verify_model_v2_custom.lum
```

Model gốc vẫn giữ nguyên:

```text
/etc/smarthome/detect_model_v2.lum
/etc/smarthome/verify_model_v2.lum
```

## Config Lưu Ở Đâu

Config cho từng HC nằm tại:

```text
hc/<mac-folder>/config.json
```

Ví dụ HC có MAC:

```text
24:95:07:e0:81:96
```

Thư mục tương ứng:

```text
hc/24-95-07-e0-81-96/
```

Config ví dụ:

```json
{
  "config_version": 4,
  "env": [
    "DETECT_MAIN_STREAM=false",
    "DETECT_TARGET_FPS=3",
    "IOU_RATE=0.8",
    "LD_LIBRARY_PATH=/usr/local/lib/",
    "MODEL_PATH=/etc/smarthome/detect_model_v2_custom.lum",
    "RECORD_MAIN_STREAM=true",
    "SCORE_BASE=0.7",
    "SCORE_SECURITY=0.8",
    "SCORE_VERIFY=0.5",
    "SECURITY_TIME_WINDOW=2",
    "VERIFY_MODEL_PATH=/etc/smarthome/verify_model_v2_custom.lum",
    "VLM_ENDPOINT=https://api-vlm-gateway.bizfly.cluster.lumi.biz/v1/runtime/resource/verify-cloud"
  ]
}
```

## Manifest Là Gì

Manifest cho từng HC nằm tại:

```text
hc/<mac-folder>/manifest.json
```

Manifest khai báo:

- Version remote.
- URL model trên GitHub raw.
- Đường dẫn model sẽ được lưu trên HC.
- File config tương ứng.
- SHA256 để script tự verify.

Ví dụ:

```json
{
  "detect": {
    "version": 3,
    "url": "https://raw.githubusercontent.com/Do-EE2-IoT/ai-ota-manager/main/model/detect_model_v2_custom.lum",
    "path": "/etc/smarthome/detect_model_v2_custom.lum",
    "sha256": "auto-generated"
  },
  "verify": {
    "version": 3,
    "url": "https://raw.githubusercontent.com/Do-EE2-IoT/ai-ota-manager/main/model/verify_model_v2_custom.lum",
    "path": "/etc/smarthome/verify_model_v2_custom.lum",
    "sha256": "auto-generated"
  },
  "config": {
    "version": 4,
    "file": "config.json",
    "sha256": "auto-generated"
  }
}
```

Không sửa `sha256` bằng tay. Chạy tool ở bước cuối để tự cập nhật.

## Thêm HC Mới Theo MAC

Ví dụ cần thêm HC có MAC:

```text
AA:BB:CC:DD:EE:FF
```

Đổi MAC thành tên thư mục:

```text
aa-bb-cc-dd-ee-ff
```

Tạo thư mục:

```bash
mkdir -p hc/aa-bb-cc-dd-ee-ff
```

Copy config và manifest mẫu từ HC đang có:

```bash
cp hc/24-95-07-e0-81-96/config.json hc/aa-bb-cc-dd-ee-ff/config.json
cp hc/24-95-07-e0-81-96/manifest.json hc/aa-bb-cc-dd-ee-ff/manifest.json
```

Sửa file:

```text
hc/aa-bb-cc-dd-ee-ff/config.json
hc/aa-bb-cc-dd-ee-ff/manifest.json
```

Với HC mới, nên bắt đầu version từ `1` nếu local `/etc/smarthome/ota_ai_info.json` chưa có version tương ứng.

## Sửa Config

File cần sửa:

```text
hc/<mac-folder>/config.json
```

Ví dụ đổi `SCORE_BASE`:

```text
SCORE_BASE=0.7
```

thành:

```text
SCORE_BASE=0.75
```

Sau đó tăng:

```json
"config_version": 4
```

thành:

```json
"config_version": 5
```

Trong manifest cũng tăng config version tương ứng:

```json
"config": {
  "version": 5,
  "file": "config.json",
  "sha256": "auto-generated"
}
```

Không sửa `sha256` bằng tay.

## Thêm Model Mới

Copy model mới vào thư mục:

```text
model/
```

Ví dụ:

```text
model/detect_model_v3_custom.lum
model/verify_model_v3_custom.lum
```

Sửa `manifest.json`:

```json
{
  "detect": {
    "version": 4,
    "url": "https://raw.githubusercontent.com/Do-EE2-IoT/ai-ota-manager/main/model/detect_model_v3_custom.lum",
    "path": "/etc/smarthome/detect_model_v3_custom.lum",
    "sha256": "auto-generated"
  },
  "verify": {
    "version": 4,
    "url": "https://raw.githubusercontent.com/Do-EE2-IoT/ai-ota-manager/main/model/verify_model_v3_custom.lum",
    "path": "/etc/smarthome/verify_model_v3_custom.lum",
    "sha256": "auto-generated"
  }
}
```

Sửa `config.json` để HC dùng đúng model mới:

```text
MODEL_PATH=/etc/smarthome/detect_model_v3_custom.lum
VERIFY_MODEL_PATH=/etc/smarthome/verify_model_v3_custom.lum
```

Tăng `config_version` và `config.version`.

## Tự Cập Nhật SHA256

Sau khi sửa config hoặc model, chạy:

```bash
python3 tools/refresh-manifest-sha.py hc/24-95-07-e0-81-96
```

Nếu là HC khác, thay bằng thư mục MAC tương ứng:

```bash
python3 tools/refresh-manifest-sha.py hc/aa-bb-cc-dd-ee-ff
```

Tool này sẽ tự cập nhật:

- `detect.sha256` nếu URL trỏ tới file trong `model/`.
- `verify.sha256` nếu URL trỏ tới file trong `model/`.
- `config.sha256` theo file `config.json`.

## Validate Trước Khi Push

Chạy:

```bash
python3 -m json.tool hc/24-95-07-e0-81-96/config.json >/tmp/config-check.json
python3 -m json.tool hc/24-95-07-e0-81-96/manifest.json >/tmp/manifest-check.json
sh -n scripts/check-model-update.sh scripts/ota-lib.sh
```

Nếu không có lỗi thì có thể push.

## Push Lên GitHub

```bash
git add model/ hc/ scripts/ tools/ README.md guide.md
git commit -m "Update AI OTA config and model"
git push
```

Sau khi push, GitHub raw có thể cache 1-3 phút. HC sẽ tự kiểm tra trong lượt cron kế tiếp.

## HC Tự Động Làm Gì Sau Khi Push

Mỗi 5 phút HC sẽ:

1. Đọc MAC trong `/etc/smarthome/mac_addr.txt`.
2. Chuyển MAC thành thư mục, ví dụ `24-95-07-e0-81-96`.
3. Tải `manifest.json` từ GitHub raw.
4. Đọc local version từ `/etc/smarthome/ota_ai_info.json`.
5. Nếu remote version không lớn hơn local version thì bỏ qua.
6. Nếu detect/verify version mới hơn, tải model về `.tmp`.
7. Verify SHA256.
8. Ghi model custom vào `path` trong manifest.
9. Nếu config version mới hơn, tải `config.json`.
10. Verify SHA256 config.
11. Apply `env` vào `/etc/smarthome/hc-config.json`.
12. Restart `aibox`.
13. Nếu `aibox` chạy lại thành công, cập nhật `/etc/smarthome/ota_ai_info.json`.
14. Nếu fail, đổi model path về model gốc và restart lại.

## Kiểm Tra Kết Quả Trên HC

Xem log:

```bash
tail -n 80 /var/log/ai-ota-manager.log
```

Xem version:

```bash
cat /etc/smarthome/ota_ai_info.json
```

Xem config đang dùng model nào:

```bash
python3 - <<'PY'
import json

with open("/etc/smarthome/hc-config.json") as f:
    env = json.load(f)["bridge_component"]["aibox"]["env"]

for key in ["MODEL_PATH", "VERIFY_MODEL_PATH", "SCORE_BASE", "SCORE_SECURITY", "SCORE_VERIFY"]:
    print(next(x for x in env if x.startswith(key + "=")))
PY
```

Xem model custom đã có chưa:

```bash
ls -lh /etc/smarthome/*custom.lum
```

## Checklist Nhanh Mỗi Lần Update

- [ ] Sửa `config.json`.
- [ ] Tăng `config_version`.
- [ ] Tăng `config.version` trong manifest.
- [ ] Nếu thay model, thêm file vào `model/`.
- [ ] Nếu thay model, sửa `url`, `path` và tăng `detect.version` hoặc `verify.version`.
- [ ] Chạy `python3 tools/refresh-manifest-sha.py hc/<mac-folder>`.
- [ ] Validate JSON.
- [ ] Commit và push.
- [ ] Chờ cron tự chạy.
- [ ] Xem log `/var/log/ai-ota-manager.log`.

