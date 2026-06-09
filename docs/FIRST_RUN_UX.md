# Pollen DEK - First Run UX & Quickstart

This guide provides a rapid end-to-end walkthrough (under 10 minutes) for deploying the Pollen DEK and enforcing your first policy.

## 1. Prerequisites
Ensure you have the latest DEK release binaries downloaded (`dek-core`, `dek-mcp-proxy`, `dek-ext-authz`, `dek-cli`).
Place them in your PATH or a working directory.

## 2. Enrollment
Link your device to the Pollen Control Plane:
```bash
dek-cli enroll --cloud-url https://pollen-cloud.example.com
```
*This exchanges a bootstrap token for mutual TLS (mTLS) identities. Certificates are stored securely.*

## 3. Status & Doctor
Verify that the device identity is valid and the control plane is reachable:
```bash
dek-cli doctor
dek-cli status
```
*The doctor command runs local diagnostics, ensuring ports are available and certs are valid.*

## 4. Run DEK Core
Start the core policy engine (runs in the background or foreground):
```bash
dek-core
```

## 5. Hot Reload & Rotate
You can dynamically push new policies from the Cloud. The DEK will hot-reload without dropping connections:
```bash
dek-cli debug reload
```
Force an identity rotation without downtime:
```bash
dek-cli debug rotate-identity
```

---

# การเริ่มต้นใช้งาน Pollen DEK ฉบับเร่งด่วน

คู่มือนี้เป็นขั้นตอนเริ่มต้นใช้งาน Pollen DEK อย่างรวดเร็ว (ภายใน 10 นาที) เพื่อใช้สำหรับตรวจสอบและบังคับใช้นโยบาย (Policy)

## 1. การเตรียมความพร้อม
ดาวน์โหลดไฟล์ไบนารีสำหรับ DEK release ล่าสุด (`dek-core`, `dek-mcp-proxy`, `dek-ext-authz`, `dek-cli`)
และวางไว้ใน PATH หรือโฟลเดอร์ทำงานของคุณ

## 2. การลงทะเบียนอุปกรณ์ (Enrollment)
เชื่อมต่ออุปกรณ์ของคุณกับ Pollen Control Plane:
```bash
dek-cli enroll --cloud-url https://pollen-cloud.example.com
```
*ขั้นตอนนี้จะนำ bootstrap token ไปแลกเป็นใบรับรอง mutual TLS (mTLS) โดยระบบจะเก็บใบรับรองไว้อย่างปลอดภัย*

## 3. ตรวจสอบสถานะ (Status & Doctor)
ตรวจสอบความถูกต้องของระบบและยืนยันการเชื่อมต่อ:
```bash
dek-cli doctor
dek-cli status
```
*คำสั่ง doctor จะวิเคราะห์ความสมบูรณ์ของระบบ พอร์ตที่ใช้ และความถูกต้องของใบรับรองต่างๆ*

## 4. เริ่มใช้งาน DEK Core
สั่งรัน policy engine หลัก:
```bash
dek-core
```

## 5. การอัปเดตแบบ Hot Reload และการเปลี่ยนใบรับรอง (Rotate)
คุณสามารถส่งนโยบายใหม่จาก Cloud มาได้ทันที DEK จะทำการโหลดข้อมูลใหม่ (hot-reload) โดยไม่ทำให้การเชื่อมต่อปัจจุบันขาดหาย:
```bash
dek-cli debug reload
```
หากต้องการหมุนเวียนใบรับรองความปลอดภัย (Rotate Identity) ทันที:
```bash
dek-cli debug rotate-identity
```
