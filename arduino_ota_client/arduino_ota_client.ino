#include <WiFi.h>
#include <HTTPClient.h>
#include <Update.h>
#include <ArduinoJson.h>

const char* ssid = "POCO X7 Pro"; // Atau WiFi Kampus, bebas!
const char* password = "88888888";

// KITA PAKAI PUBLIC TUNNEL! MENGHUBUNGKAN ESP32 KE LOCALHOST-MU DARI MANAPUN
const String server_url = "http://liill-103-94-190-243.free.pinggy.net"; 
const String device_id = "ESP32_G09";
const String admin_token = "admin_iot_g09_rahasia_1234567890";

String current_version = "v1.0.0"; 
unsigned long lastCheck = 0;

void setup() {
  Serial.begin(115200);
  WiFi.begin(ssid, password);
  Serial.print("Connecting to WiFi");
  while (WiFi.status() != WL_CONNECTED) {
    delay(500); Serial.print(".");
  }
  Serial.println("\nConnected! IP: " + WiFi.localIP().toString());
}

void loop() {
  static unsigned long lastPrint = 0;
  if (millis() - lastPrint > 2000) {
    Serial.println(">>> [V1] SAYA ADALAH PROGRAM VERSI 1 (LAMA) <<<");
    lastPrint = millis();
  }

  if (millis() - lastCheck > 10000) {
    if (WiFi.status() == WL_CONNECTED) checkAndDownloadOTA();
    lastCheck = millis();
  }
}

void checkAndDownloadOTA() {
  HTTPClient http;
  String checkUrl = server_url + "/api/ota/check?target=" + device_id;
  Serial.println("\n[OTA] Mengecek versi baru ke Public Tunnel: " + checkUrl);
  
  http.begin(checkUrl);
  http.addHeader("Authorization", "Bearer " + admin_token);
  
  int httpCode = http.GET();
  
  if (httpCode > 0) {
    if (httpCode == HTTP_CODE_OK) {
      String payload = http.getString();
      JsonDocument doc;
      DeserializationError err = deserializeJson(doc, payload);
      
      if (!err && !doc["candidate"].isNull()) {
        String new_version = doc["candidate"]["firmware_version"].as<String>();
        String download_url = doc["candidate"]["download_url"].as<String>();
        
        Serial.println("[OTA] Versi saat ini: " + current_version + " | Versi Server: " + new_version);
        
        if (new_version != current_version && new_version != "null") {
          Serial.println("[OTA] Ada update! Mulai mengunduh...");
          performUpdate(server_url + download_url);
        } else {
          Serial.println("[OTA] Firmware sudah sama/paling baru.");
        }
      } else {
        Serial.println("[OTA] Kandidat belum ada di Server.");
      }
    } else {
      Serial.printf("[OTA] Server merespons dengan HTTP Error: %d\n", httpCode);
      if (httpCode == 401) Serial.println("-> Cek TOKEN ADMIN!");
    }
  } else {
    Serial.printf("[OTA] Gagal menghubungi server tunnel, error: %s\n", http.errorToString(httpCode).c_str());
  }
  http.end();
}

void performUpdate(String url) {
  HTTPClient http;
  http.begin(url);
  http.addHeader("Authorization", "Bearer " + admin_token);
  
  int httpCode = http.GET();
  if (httpCode == HTTP_CODE_OK) {
    int contentLength = http.getSize();
    if (contentLength > 0 && Update.begin(contentLength)) {
      Serial.println("[OTA] Flashing firmware baru dari Tunnel. Tunggu...");
      WiFiClient& client = http.getStream();
      size_t written = Update.writeStream(client);
      if (written == contentLength && Update.end()) {
        Serial.println("[OTA] SUKSES! Restarting ESP32...");
        ESP.restart();
      } else {
        Serial.printf("[OTA] Gagal Flash. Error #: %d\n", Update.getError());
      }
    } else {
      Serial.println("[OTA] Partisi gagal dibuka atau ukuran tidak valid.");
    }
  } else {
    Serial.printf("[OTA] Download dari Tunnel gagal, HTTP code: %d\n", httpCode);
  }
  http.end();
}
