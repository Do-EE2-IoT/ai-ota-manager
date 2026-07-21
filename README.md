# Model OTA Repository

Repository này dùng để quản lý OTA cho model AI và cấu hình của Home Controller (HC).

Repository **không chứa source code của HC** và **không lưu file model `.lum` trong Git history**. Git chỉ lưu manifest, file config, tài liệu và script/tool phục vụ OTA. File model thật nên được lưu ở GitHub Release Asset, object storage, CDN hoặc server nội bộ có HTTPS.

## Mục Tiêu

- OTA model theo từng HC riêng biệt.
- Cập nhật được `detect model`, `verify model` và `config`.
- Không ảnh hưởng HC khác khi chỉ cập nhật một HC cụ thể.
- Có kiểm tra version, SHA256, backup, rollback và health check.
- Tránh commit model lớn vào Git.

## Nguyên Tắc Thiết Kế

1. Mỗi HC có một thư mục OTA riêng theo MAC address.
2. `manifest.json` là nguồn dữ liệu OTA từ xa.
3. `ota_ai_info.json` trên HC là nguồn dữ liệu version cục bộ.
4. Version là căn cứ duy nhất để quyết định có update hay không.
5. SHA256 chỉ dùng để kiểm tra tính toàn vẹn của file sau khi download hoặc đọc từ repo.
6. Config OTA dùng file đầy đủ các trường `env` cần quản lý. Khi `config.version` tăng, HC kiểm tra SHA256 rồi apply toàn bộ mảng `env` hợp lệ.
7. Model được download về file `.tmp`, verify xong mới replace.
8. Nếu update lỗi, hệ thống rollback về bản trước đó.

## Cấu Trúc Repository

```text
model-ota/
├── README.md
├── .gitignore
├── docs/
│   ├── OTA_FLOW.md
│   ├── OTA_MANIFEST.md
│   └── OTA_VERSION.md
├── scripts/
│   ├── check-model-update.sh
│   └── ota-lib.sh
├── tools/
│   ├── calc-sha256.sh
│   ├── create-manifest.py
│   └── validate-manifest.py
└── hc/
    └── 24-95-07-e0-81-96/
        ├── manifest.json
        └── config.json
```

| Đường dẫn | Mục đích |
| --- | --- |
| `README.md` | Tài liệu tổng quan repository |
| `docs/` | Tài liệu chi tiết về OTA flow, manifest và version |
| `scripts/` | Script chạy trên HC để kiểm tra và thực hiện OTA |
| `tools/` | Tool chạy trên máy phát triển để tạo và kiểm tra manifest |
| `hc/` | Dữ liệu OTA riêng cho từng HC |

## Quy Ước Thư Mục HC

Mỗi HC được định danh bằng MAC address.

Ví dụ MAC:

```text
24:95:07:E0:81:96
```

Tên thư mục tương ứng:

```text
hc/24-95-07-e0-81-96/
```

Quy tắc chuyển đổi:

- Chuyển toàn bộ chữ cái sang chữ thường.
- Thay dấu `:` bằng dấu `-`.

| MAC address | Thư mục OTA |
| --- | --- |
| `24:95:07:E0:81:96` | `hc/24-95-07-e0-81-96/` |
| `AA:BB:CC:DD:EE:FF` | `hc/aa-bb-cc-dd-ee-ff/` |

## Manifest OTA

Mỗi HC có một file manifest riêng:

```text
hc/<mac-address>/manifest.json
```

Ví dụ:

```json
{
  "detect": {
    "version": 3,
    "url": "https://example.com/models/detect_model_v3.lum",
    "sha256": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  },
  "verify": {
    "version": 2,
    "url": "https://example.com/models/verify_model_v2.lum",
    "sha256": "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"
  },
  "config": {
    "version": 5,
    "file": "config.json",
    "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  }
}
```

| Trường | Bắt buộc | Ý nghĩa |
| --- | --- | --- |
| `detect.version` | Có, nếu OTA detect model | Version của detect model |
| `detect.url` | Có, nếu OTA detect model | URL tải detect model |
| `detect.sha256` | Có, nếu OTA detect model | SHA256 của detect model |
| `verify.version` | Có, nếu OTA verify model | Version của verify model |
| `verify.url` | Có, nếu OTA verify model | URL tải verify model |
| `verify.sha256` | Có, nếu OTA verify model | SHA256 của verify model |
| `config.version` | Có, nếu OTA config | Version của config |
| `config.file` | Có, nếu OTA config | Tên file config trong cùng thư mục HC |
| `config.sha256` | Có, nếu OTA config | SHA256 của file config để kiểm tra hợp lệ |

Một HC không bắt buộc phải có đủ cả `detect`, `verify` và `config`. Nếu HC chỉ cần cập nhật config thì manifest có thể chỉ chứa phần `config`.

## Quy Tắc Version

Version là số nguyên tăng dần.

HC chỉ update khi:

```text
remote_version > local_version
```

HC không update khi:

```text
remote_version <= local_version
```

Không dùng SHA256 để quyết định có update hay không. SHA256 chỉ dùng để xác thực file sau khi download hoặc đọc từ repo.

## Config OTA

File config OTA đặt tại:

```text
hc/<mac-address>/config.json
```

Config nên khai báo đầy đủ các trường `env` mà HC cần quản lý. Khi muốn chỉnh tham số, chỉ sửa value của dòng tương ứng, tính lại SHA256 của file config và tăng `config.version` trong `manifest.json`.

Ví dụ:

```json
{
  "config_version": 5,
  "env": [
    "DETECT_MAIN_STREAM=false",
    "DETECT_TARGET_FPS=3",
    "IOU_RATE=0.8",
    "LD_LIBRARY_PATH=/usr/local/lib/",
    "MODEL_PATH=/etc/smarthome/detect_model_v2.lum",
    "RECORD_MAIN_STREAM=true",
    "SCORE_BASE=0.6",
    "SCORE_SECURITY=0.8",
    "SCORE_VERIFY=0.5",
    "SECURITY_TIME_WINDOW=2",
    "VERIFY_MODEL_PATH=/etc/smarthome/verify_model_v2.lum",
    "VLM_ENDPOINT=https://api-vlm-gateway.bizfly.cluster.lumi.biz/v1/runtime/resource/verify-cloud"
  ]
}
```

### Cách Chỉnh Tham Số

Ví dụ muốn đổi ngưỡng `SCORE_BASE` từ `0.6` lên `0.7`:

```json
{
  "config_version": 6,
  "env": [
    "DETECT_MAIN_STREAM=false",
    "DETECT_TARGET_FPS=3",
    "IOU_RATE=0.8",
    "LD_LIBRARY_PATH=/usr/local/lib/",
    "MODEL_PATH=/etc/smarthome/detect_model_v2.lum",
    "RECORD_MAIN_STREAM=true",
    "SCORE_BASE=0.7",
    "SCORE_SECURITY=0.8",
    "SCORE_VERIFY=0.5",
    "SECURITY_TIME_WINDOW=2",
    "VERIFY_MODEL_PATH=/etc/smarthome/verify_model_v2.lum",
    "VLM_ENDPOINT=https://api-vlm-gateway.bizfly.cluster.lumi.biz/v1/runtime/resource/verify-cloud"
  ]
}
```

Sau đó cập nhật manifest:

```json
{
  "config": {
    "version": 6,
    "file": "config.json",
    "sha256": "<sha256-moi-cua-file-config-json>"
  }
}
```

HC sẽ:

1. Đọc `config.version` từ manifest.
2. So sánh với `config_version` local.
3. Nếu remote version lớn hơn local version, tải hoặc đọc `config.json`.
4. Tính SHA256 của `config.json`.
5. So sánh với `config.sha256` trong manifest.
6. Nếu SHA256 hợp lệ, parse `env` và áp dụng các biến môi trường vào cấu hình chạy của HC.
7. Restart service hoặc AIBOX nếu cần.
8. Health check.
9. Chỉ cập nhật local version khi health check pass.

## Local Version Trên HC

HC lưu trạng thái OTA cục bộ tại:

```text
/etc/smarthome/ota_ai_info.json
```

Ví dụ:

```json
{
  "detect_version": 3,
  "verify_version": 2,
  "config_version": 5,
  "last_update": "2026-07-21T10:20:00Z",
  "status": "success"
}
```

File này là dữ liệu duy nhất dùng để so sánh version với manifest từ xa.

## Model Storage

Không lưu file `.lum` trong Git repository.

Nên lưu model ở một trong các nơi sau:

- GitHub Release Asset
- S3-compatible object storage
- Server nội bộ có HTTPS
- CDN hoặc artifact storage riêng

Manifest chỉ lưu URL download và SHA256.

Ví dụ:

```json
{
  "detect": {
    "version": 3,
    "url": "https://github.com/<org>/<repo>/releases/download/model-v3/detect_model_v3.lum",
    "sha256": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  }
}
```

## OTA Workflow

```text
Cron
  ↓
Đọc MAC address của HC
  ↓
Chuyển MAC thành tên thư mục OTA
  ↓
GET hc/<mac>/manifest.json
  ↓
Nếu 404
  ↓
Thoát, không update
  ↓
Nếu manifest tồn tại
  ↓
Đọc local version từ /etc/smarthome/ota_ai_info.json
  ↓
So sánh remote version và local version
  ↓
Nếu remote_version <= local_version
  ↓
Thoát, không update
  ↓
Nếu remote_version > local_version
  ↓
Download model hoặc đọc config về dạng .tmp
  ↓
Kiểm tra SHA256
  ↓
Kiểm tra model có thể load nếu update model
  ↓
Backup file cũ
  ↓
Replace file mới hoặc apply config mới
  ↓
Restart service hoặc AIBOX
  ↓
Health check
  ↓
Nếu PASS
  ↓
Cập nhật ota_ai_info.json
  ↓
Nếu FAIL
  ↓
Rollback
```

## Script OTA

Script chính:

```text
scripts/check-model-update.sh
```

Trách nhiệm:

- Đọc MAC của HC.
- Tải manifest tương ứng.
- Đọc version cục bộ.
- So sánh version.
- Gọi các hàm download, verify, replace, restart, health check và rollback.

Thư viện hàm chung:

```text
scripts/ota-lib.sh
```

Nên chứa các nhóm hàm:

- Logging
- Download file
- Verify SHA256
- Validate manifest
- Backup file cũ
- Replace file
- Apply config
- Restart service
- Health check
- Rollback
- Update local version

Không nên viết toàn bộ logic trong một file duy nhất.

## Tool Hỗ Trợ

Các tool trong thư mục:

```text
tools/
```

### `calc-sha256.sh`

Dùng để tính SHA256 của một file.

Ví dụ:

```bash
./tools/calc-sha256.sh hc/24-95-07-e0-81-96/config.json
./tools/calc-sha256.sh detect_model_v3.lum
```

### `create-manifest.py`

Dùng để tạo manifest tự động.

Nên hỗ trợ:

- Kiểm tra file model hoặc file config tồn tại.
- Tính SHA256.
- Nhận version.
- Nhận URL public của model.
- Sinh `manifest.json`.

### `validate-manifest.py`

Dùng để kiểm tra manifest trước khi commit.

Nên kiểm tra:

- JSON hợp lệ.
- Version là số nguyên dương.
- URL tồn tại với model OTA.
- SHA256 có đúng định dạng 64 ký tự hex.
- File `config.json` tồn tại nếu manifest khai báo `config.file`.
- SHA256 của `config.json` khớp với `config.sha256`.
- `env` là mảng string theo định dạng `KEY=VALUE`.
- Các biến bắt buộc trong `env` không bị thiếu.

## Quy Trình Tạo Repository Ban Đầu

```bash
mkdir model-ota
cd model-ota
git init
mkdir -p docs scripts tools hc
touch README.md .gitignore
git add .
git commit -m "Initialize OTA repository"
```

## Không Upload Model Ngay Từ Đầu

Giai đoạn đầu nên để thư mục `hc/` trống.

Không upload file `.lum` trước khi OTA framework hoàn chỉnh.

Lý do:

- Chưa có manifest chuẩn.
- Chưa có script OTA ổn định.
- Chưa test rollback.
- Chưa test health check.
- Dễ tạo Git history chứa model lớn không cần thiết.

Chỉ upload model thật sau khi toàn bộ OTA framework đã được test ổn định bằng file giả lập.

## Test OTA Bằng File Giả Lập

Không test OTA lần đầu bằng model thật.

Nên tạo file giả lập:

```text
hello.txt
```

Dùng `hello.txt` thay cho:

```text
detect_model_v1.lum
```

Mục tiêu test:

- Download
- Verify SHA256
- Replace file
- Backup
- Rollback
- Apply config
- Restart
- Health check
- Update local version

Sau khi flow chạy ổn định mới thay bằng model thật.

## Quy Trình OTA Cho Một HC

Ví dụ cần OTA cho HC:

```text
24:95:07:E0:81:96
```

Tạo thư mục:

```text
hc/24-95-07-e0-81-96/
```

Thêm các file:

```text
hc/24-95-07-e0-81-96/manifest.json
hc/24-95-07-e0-81-96/config.json
```

Commit và push:

```bash
git add hc/24-95-07-e0-81-96/
git commit -m "Add OTA manifest for HC 24-95-07-e0-81-96"
git push
```

HC tương ứng sẽ tự kiểm tra và OTA theo lịch cron.

Cập nhật này không ảnh hưởng bất kỳ HC nào khác vì mỗi HC chỉ đọc manifest trong thư mục MAC của chính nó.

## Cron Trên HC

Khuyến nghị chạy kiểm tra OTA mỗi 30 phút.

```cron
*/30 * * * * /opt/smarthome/model-ota/scripts/check-model-update.sh >> /var/log/model-ota.log 2>&1
```

Đường dẫn script thực tế có thể thay đổi theo cách deploy trên HC.

## Chiến Lược Commit

Khuyến nghị chia commit nhỏ theo từng nhóm thay đổi:

```text
Commit 1: Initialize OTA repository
Commit 2: Add OTA documents
Commit 3: Add manifest specification
Commit 4: Add OTA scripts
Commit 5: Add OTA tools
Commit 6: Support per-HC OTA
Commit 7: Add OTA manifest for specific HC
```

Không nên commit nhiều thay đổi lớn không liên quan trong cùng một commit.

## Push Lên GitHub

Khuyến nghị dùng private repository.

```bash
git remote add origin <git-url>
git branch -M main
git push -u origin main
```

Repository chỉ nên cấp quyền read cho HC.

Nếu HC cần pull trực tiếp từ GitHub, nên dùng deploy key hoặc token có quyền tối thiểu.

## Checklist Trước Khi OTA Model Thật

- [ ] Repository không chứa file `.lum` trong Git history.
- [ ] Manifest JSON hợp lệ.
- [ ] URL model tải được từ HC.
- [ ] SHA256 trong manifest khớp với file model.
- [ ] File `config.json` tồn tại nếu OTA config.
- [ ] SHA256 của `config.json` khớp với `config.sha256`.
- [ ] Local version trên HC đọc được.
- [ ] Script xử lý đúng trường hợp manifest `404`.
- [ ] Script bỏ qua update khi remote version không lớn hơn local version.
- [ ] File được download hoặc đọc về `.tmp` trước khi replace.
- [ ] Có backup trước khi replace.
- [ ] Có rollback khi health check fail.
- [ ] Config được apply đúng các biến trong `env`.
- [ ] Log OTA đủ để debug.
- [ ] Đã test bằng file giả lập trước khi dùng model thật.

## Checklist Khi Thêm OTA Cho HC Mới

- [ ] Chuyển MAC sang đúng format thư mục.
- [ ] Tạo thư mục trong `hc/`.
- [ ] Tạo `manifest.json`.
- [ ] Tạo `config.json` nếu cần update config.
- [ ] Tính SHA256 cho `config.json`.
- [ ] Điền `config.sha256` vào manifest.
- [ ] Validate manifest.
- [ ] Commit riêng cho HC đó.
- [ ] Push lên remote.
- [ ] Theo dõi log OTA trên HC.

## Quy Tắc An Toàn

- Không commit model `.lum`.
- Không sửa manifest bằng tay nếu đã có tool `create-manifest.py`.
- Không giảm version sau khi đã phát hành.
- Không dùng cùng một version cho hai nội dung model khác nhau.
- Không update local version nếu health check chưa pass.
- Không xóa backup trước khi xác nhận bản mới chạy ổn định.
- Không apply config nếu SHA256 không khớp.
- Không để thiếu biến bắt buộc trong `env`.

## Trạng Thái Hiện Tại Của Repository

Repository này đang ở giai đoạn thiết kế OTA framework.

Thứ tự triển khai khuyến nghị:

1. Tạo cấu trúc thư mục chuẩn.
2. Viết tài liệu chi tiết trong `docs/`.
3. Viết tool tạo và validate manifest.
4. Viết script OTA.
5. Test bằng file giả lập.
6. Bổ sung manifest cho từng HC.
7. Upload model thật lên release asset hoặc object storage.
8. Bật cron OTA trên HC.
