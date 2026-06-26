# คู่มือการติดตั้ง Pollek Local Enforcement Kit (v1.0.0-beta)

## ความต้องการของระบบ

- ระบบปฏิบัติการ: Windows 10/11, macOS 12+, หรือ Ubuntu 20.04+
- พื้นที่จัดเก็บ: ว่าง 100MB
- สิทธิ์: ต้องการสิทธิ์ Administrator/root

> **หมายเหตุสำหรับ Simple Mode**: หากคุณใช้งาน Pollek ใน **Simple Mode** คุณ**ไม่ต้อง**ตั้งค่า PEP (เช่น eBPF หรือ WFP) ที่ซับซ้อนใดๆ ระบบจะจัดการเรื่อง Enforcement ให้ทำงานอัตโนมัติตาม OS และสิทธิ์ที่คุณมีอย่างแนบเนียน

## Preflight Check

รันเครื่องมือ doctor ก่อนทำการติดตั้งเพื่อตรวจสอบว่าระบบของคุณรองรับหรือไม่:

```bash
pollek-dekctl doctor
```

## การติดตั้งบน Windows

1. ดาวน์โหลด `Pollek-dek-x86_64-pc-windows-msvc.msi` จากหน้า GitHub Releases
2. ดับเบิลคลิกไฟล์ MSI เพื่อติดตั้งตามขั้นตอน
3. Service ชื่อ `PollekDEKCore` จะถูกติดตั้งและเริ่มทำงานโดยอัตโนมัติ

## การติดตั้งบน Linux

1. ดาวน์โหลดไฟล์ `.deb` ให้ตรงกับสถาปัตยกรรม (เช่น `Pollek-dek-x86_64-unknown-linux-gnu.deb` หรือ `aarch64`)
2. รันคำสั่งติดตั้ง: `sudo dpkg -i Pollek-dek-*.deb`
3. systemd service ชื่อ `Pollek-Local Enforcement Kit.service` จะเริ่มทำงานโดยอัตโนมัติ

## การติดตั้งบน macOS

1. ดาวน์โหลดไฟล์ `.pkg` (เช่น `Pollek-dek-x86_64-apple-darwin.pkg`)
2. รันตัวติดตั้ง package
3. launchd agent ชื่อ `ai.Pollek.Local Enforcement Kit` จะโหลดและทำงานโดยอัตโนมัติ

## การตรวจสอบ

รันคำสั่ง `Pollek-dekctl status` เพื่อตรวจสอบสถานะการติดตั้งและการทำงานของ service
