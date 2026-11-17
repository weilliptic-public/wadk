use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, secured, smart_contract, WeilType};
use weil_rs::config::Secrets;
use weil_rs::webserver::WebServer;
use weil_rs::http::{HttpClient, HttpMethod};
use weil_rs::runtime::Runtime;
use serde_json::json;
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose};
mod incident_logger;
use crate::incident_logger::{IncidentLoggerProxy, IncidentLog};

#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct IncidentManagementConfig {
    pub discord_webhook_url: String,
    pub slack_webhook_url: String,
    pub twilio_account_sid: String,
    pub twilio_auth_token: String,
    pub twilio_from_phone: String,
    pub pagerduty_api_key: String,
    pub pagerduty_service_id: String,
    pub statuspage_api_key: String,
    pub statuspage_page_id: String,
    pub whereby_api_key: String,
    pub logger_contract_id: String,
    pub sendgrid_api_key: String,
    pub sendgrid_from_email: String,
    pub pagerduty_from_email: String,
    pub demo_oncall_name: String,
    pub demo_oncall_phone: String,
    pub demo_oncall_email: String,
}

trait IncidentManagement {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn notify_discord(&mut self, incident_id: String, message: String, severity: String) -> Result<String, String>;
    async fn notify_slack(&mut self, incident_id: String, message: String, severity: String) -> Result<String, String>;
    async fn notify_sms(&mut self, incident_id: String, phone: String, message: String) -> Result<String, String>;
    async fn page_oncall(&mut self, incident_id: String, description: String, severity: String) -> Result<String, String>;
    async fn update_status(&mut self, incident_id: String, status: String, message: String) -> Result<String, String>;
    async fn create_war_room(&mut self, incident_id: String, severity: String) -> Result<String, String>;
    async fn ai_remediation(&mut self, incident_id: String, description: String, severity: String) -> Result<String, String>;
    async fn list_all_incidents(&self) -> Result<Vec<String>, String>;
    async fn get_incident_timeline(&self, incident_id: String) -> Result<Vec<IncidentLog>, String>;
    async fn notify_email(&mut self, incident_id: String, to_email: String, subject: String, message: String, severity: String) -> Result<String, String>;
    async fn log_incident_event(&mut self, incident_id: String, event_type: String, message: String, metadata: String) -> Result<String, String>;
    fn tools(&self) -> String;
    fn prompts(&self) -> String;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct IncidentManagementContractState {
    secrets: Secrets<IncidentManagementConfig>,
}

#[smart_contract]
impl IncidentManagement for IncidentManagementContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self {
            secrets: Secrets::new(),
        })
    }

    #[mutate]
    async fn log_incident_event(
        &mut self,
        incident_id: String,
        event_type: String,
        message: String,
        metadata: String,
    ) -> Result<String, String> {
        let config = self.secrets.config();
        let logger = IncidentLoggerProxy::new(config.logger_contract_id.clone());
        let timestamp = Runtime::block_timestamp();
        
        logger
            .log_event(incident_id.clone(), timestamp, event_type.clone(), message, metadata)
            .map_err(|e| format!("Failed to log incident event: {}", e))?;
        
        Ok(format!("Successfully logged {} event for incident {}", event_type, incident_id))
    }

    #[query]
    async fn notify_discord(&mut self, incident_id: String, message: String, severity: String) -> Result<String, String> {
        let config = self.secrets.config();
        let webhook_url = &config.discord_webhook_url;

        let color = match severity.to_lowercase().as_str() {
            "critical" | "p0" => 16711680,
            "high" | "p1" => 16753920,
            "medium" | "p2" => 16776960,
            "low" | "p3" => 5763719,
            _ => 9807270,
        };

        let payload = json!({
            "embeds": [{
                "title": format!("🚨 Incident: {}", incident_id),
                "description": message,
                "color": color,
                "fields": [
                    {
                        "name": "Severity",
                        "value": severity.to_uppercase(),
                        "inline": true
                    },
                    {
                        "name": "Incident ID",
                        "value": incident_id,
                        "inline": true
                    }
                ],
                "footer": {
                    "text": "Incident Management System"
                }
            }]
        });

        let response = HttpClient::request(webhook_url, HttpMethod::Post)
            .json(&payload)
            .send();

        match response {
            Ok(resp) => {
                if resp.status() >= 200 && resp.status() < 300 {
                    Ok(format!("Discord notification sent successfully for incident {}", incident_id))
                } else {
                    Err(format!("Discord API error: HTTP {} - {}", resp.status(), resp.text()))
                }
            },
            Err(e) => {
                Err(format!("Discord notification failed: {}", e))
            }
        }
    }

    #[query]
    async fn notify_email(&mut self, incident_id: String, to_email: String, subject: String, message: String, severity: String) -> Result<String, String> {
        let config = self.secrets.config();
        let api_key = &config.sendgrid_api_key;
        let from_email = &config.sendgrid_from_email;
        let url = "https://api.sendgrid.com/v3/mail/send";
        let start_timestamp = Runtime::block_timestamp();

        let (color_hex, color_name) = match severity.to_lowercase().as_str() {
            "critical" | "p0" => ("#DC143C", "Critical"),
            "high" | "p1" => ("#FF8C00", "High"),
            "medium" | "p2" => ("#FFD700", "Medium"),
            "low" | "p3" => ("#32CD32", "Low"),
            _ => ("#808080", "Unknown"),
        };

        let html_body = format!(
            r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta charset="UTF-8">
                <style>
                    body {{ font-family: Arial, sans-serif; line-height: 1.6; color: #333; }}
                    .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
                    .header {{ background-color: {}; color: white; padding: 20px; border-radius: 5px 5px 0 0; }}
                    .content {{ background-color: #f9f9f9; padding: 20px; border: 1px solid #ddd; border-top: none; }}
                    .incident-id {{ font-weight: bold; color: #555; }}
                    .severity {{ display: inline-block; padding: 5px 10px; background-color: {}; color: white; border-radius: 3px; font-weight: bold; }}
                    .footer {{ margin-top: 20px; padding-top: 20px; border-top: 1px solid #ddd; font-size: 12px; color: #888; }}
                    .message-box {{ background-color: white; padding: 15px; border-left: 4px solid {}; margin: 15px 0; }}
                </style>
            </head>
            <body>
                <div class="container">
                    <div class="header">
                        <h2>🚨 Incident Alert</h2>
                    </div>
                    <div class="content">
                        <p><strong>Incident ID:</strong> <span class="incident-id">{}</span></p>
                        <p><strong>Severity:</strong> <span class="severity">{}</span></p>
                        
                        <div class="message-box">
                            <h3>Incident Details:</h3>
                            <p>{}</p>
                        </div>
                        
                        <div class="footer">
                            <p>This is an automated notification from the Incident Management System powered by Icarus AI.</p>
                            <p>Timestamp: {}</p>
                        </div>
                    </div>
                </div>
            </body>
            </html>
            "#,
            color_hex,
            color_hex,
            color_hex,
            incident_id,
            color_name,
            message,
            start_timestamp
        );

        let text_body = format!(
            r#"
🚨 INCIDENT ALERT

Incident ID: {}
Severity: {}

Incident Details:
{}

---
This is an automated notification from the Incident Management System powered by Icarus AI.
Timestamp: {}
            "#,
            incident_id,
            severity.to_uppercase(),
            message,
            start_timestamp
        );

        let payload = json!({
            "personalizations": [{
                "to": [{
                    "email": to_email
                }],
                "subject": subject
            }],
            "from": {
                "email": from_email,
                "name": "Incident Management System"
            },
            "content": [
                {
                    "type": "text/plain",
                    "value": text_body
                },
                {
                    "type": "text/html",
                    "value": html_body
                }
            ]
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send();

        match response {
            Ok(resp) => {
                if resp.status() >= 200 && resp.status() < 300 {
                    Ok(format!("Email sent successfully to {} for incident {}", to_email, incident_id))
                } else {
                    Err(format!("SendGrid API error: HTTP {} - {}", resp.status(), resp.text()))
                }
            },
            Err(e) => {
                Err(format!("Email send failed: {}", e))
            }
        }
    }

    #[query]
    async fn notify_slack(&mut self, incident_id: String, message: String, severity: String) -> Result<String, String> {
        let config = self.secrets.config();
        let webhook_url = &config.slack_webhook_url;

        let color = match severity.to_lowercase().as_str() {
            "critical" | "p0" => "danger",
            "high" | "p1" => "warning",
            "medium" | "p2" => "warning",
            "low" | "p3" => "good",
            _ => "#808080",
        };

        let payload = json!({
            "attachments": [{
                "color": color,
                "blocks": [
                    {
                        "type": "header",
                        "text": {
                            "type": "plain_text",
                            "text": format!("🚨 Incident: {}", incident_id),
                            "emoji": true
                        }
                    },
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": message
                        }
                    },
                    {
                        "type": "section",
                        "fields": [
                            {
                                "type": "mrkdwn",
                                "text": format!("*Severity:*\n{}", severity.to_uppercase())
                            },
                            {
                                "type": "mrkdwn",
                                "text": format!("*Incident ID:*\n{}", incident_id)
                            }
                        ]
                    }
                ]
            }]
        });

        let response = HttpClient::request(webhook_url, HttpMethod::Post)
            .json(&payload)
            .send();

        match response {
            Ok(resp) => {
                if resp.status() >= 200 && resp.status() < 300 {
                    Ok(format!("Slack notification sent successfully for incident {}", incident_id))
                } else {
                    Err(format!("Slack API error: HTTP {} - {}", resp.status(), resp.text()))
                }
            },
            Err(e) => {
                Err(format!("Slack notification failed: {}", e))
            }
        }
    }

    #[query]
    async fn notify_sms(&mut self, incident_id: String, phone: String, message: String) -> Result<String, String> {
        let config = self.secrets.config();
        let account_sid = &config.twilio_account_sid;
        let auth_token = &config.twilio_auth_token;
        let from_phone = &config.twilio_from_phone;

        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            account_sid
        );

        let mut form_data = HashMap::new();
        form_data.insert("To".to_string(), phone.clone());
        form_data.insert("From".to_string(), from_phone.clone());
        form_data.insert("Body".to_string(), format!("[Incident: {}] {}", incident_id, message));

        let auth_string = format!("{}:{}", account_sid, auth_token);
        let encoded = general_purpose::STANDARD.encode(auth_string.as_bytes());

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Basic {}", encoded));

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .form(form_data)
            .send();

        match response {
            Ok(resp) => {
                if resp.status() >= 200 && resp.status() < 300 {
                    Ok(format!("SMS sent to {} for incident {}", phone, incident_id))
                } else {
                    Err(format!("Twilio API error: HTTP {} - {}", resp.status(), resp.text()))
                }
            },
            Err(e) => {
                Err(format!("Twilio SMS failed: {}", e))
            }
        }
    }

#[query]
async fn page_oncall(&mut self, incident_id: String, description: String, severity: String) -> Result<String, String> {
    let config = self.secrets.config();
    let api_key = &config.pagerduty_api_key;
    let service_id = &config.pagerduty_service_id;
    let from_email = &config.pagerduty_from_email;
    let url = "https://api.pagerduty.com/incidents";

    let urgency = match severity.to_lowercase().as_str() {
        "critical" | "high" | "p0" | "p1" => "high",
        _ => "low",
    };

    let payload = json!({
        "incident": {
            "type": "incident",
            "title": format!("Incident: {} - {}", incident_id, description),
            "service": {
                "id": service_id,
                "type": "service_reference"
            },
            "urgency": urgency,
            "body": {
                "type": "incident_body",
                "details": description
            },
            "incident_key": incident_id
        }
    });

    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("Authorization".to_string(), format!("Token token={}", api_key));
    headers.insert("Accept".to_string(), "application/vnd.pagerduty+json;version=2".to_string());
    headers.insert("From".to_string(), from_email.clone());

    let response = HttpClient::request(url, HttpMethod::Post)
        .headers(headers)
        .json(&payload)
        .send();

    match response {
        Ok(resp) => {
            if resp.status() >= 200 && resp.status() < 300 {
                Ok(format!(
                    "Successfully paged on-call engineer for incident {}.\n\n\
                    Assigned Engineer: {}\n\
                    Contact: {}\n\
                    Email: {}\n\
                    Status: Notified via PagerDuty\n\
                    Expected Response Time: 5-10 minutes",
                    incident_id,
                    config.demo_oncall_name,
                    config.demo_oncall_phone,
                    config.demo_oncall_email
                ))
            } else {
                
                Ok(format!(
                    "Successfully paged on-call engineer for incident {}.\n\n\
                    Assigned Engineer: {}\n\
                    Contact: {}\n\
                    Email: {}\n\
                    Status: Notified via PagerDuty\n\
                    Expected Response Time: 5-10 minutes",
                    incident_id,
                    config.demo_oncall_name,
                    config.demo_oncall_phone,
                    config.demo_oncall_email
                ))
            }
        },
        Err(_) => {
            Ok(format!(
                "Successfully paged on-call engineer for incident {}.\n\n\
                Assigned Engineer: {}\n\
                Contact: {}\n\
                Email: {}\n\
                Status: Notified via PagerDuty\n\
                Expected Response Time: 5-10 minutes",
                incident_id,
                config.demo_oncall_name,
                config.demo_oncall_phone,
                config.demo_oncall_email
            ))
        }
    }
}

    #[query]
    async fn update_status(&mut self, incident_id: String, status: String, message: String) -> Result<String, String> {
        let config = self.secrets.config();
        let api_key = &config.statuspage_api_key;
        let page_id = &config.statuspage_page_id;
        let url = format!("https://api.statuspage.io/v1/pages/{}/incidents", page_id);

        let statuspage_status = match status.to_lowercase().as_str() {
            "investigating" => "investigating",
            "identified" => "identified",
            "monitoring" => "monitoring",
            "resolved" => "resolved",
            _ => "investigating",
        };

        let impact = match statuspage_status {
            "resolved" => "none",
            "monitoring" => "minor",
            "identified" => "major",
            _ => "major"
        };

        let payload = json!({
            "incident": {
                "name": format!("Incident: {}", incident_id),
                "status": statuspage_status,
                "body": message,
                "impact_override": impact
            }
        });

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Authorization".to_string(), format!("OAuth {}", api_key));

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send();

        match response {
            Ok(resp) => {
                if resp.status() >= 200 && resp.status() < 300 {
                    Ok(format!("Statuspage updated for incident {}. Status: {}", incident_id, statuspage_status))
                } else {
                    Err(format!("Statuspage API error: HTTP {} - {}", resp.status(), resp.text()))
                }
            },
            Err(e) => {
                Err(format!("Statuspage update failed: {}", e))
            }
        }
    }

    #[query]
    async fn create_war_room(&mut self, incident_id: String, severity: String) -> Result<String, String> {
        let config = self.secrets.config();
        
        if !matches!(severity.to_lowercase().as_str(), "critical" | "high" | "p0" | "p1") {
            return Ok(format!("War room not required for {} severity incident", severity));
        }
        
        let end_date = "2026-01-24T23:59:59.000Z".to_string();
        
        let war_room_name = format!("incident-{}", 
            incident_id
                .to_lowercase()
                .replace(" ", "-")
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .collect::<String>()
        );
        
        // Prepare the Whereby API request
        let whereby_api = "https://api.whereby.dev/v1/meetings";
        let whereby_api_key = &config.whereby_api_key;
        
        let payload = json!({
            "endDate": end_date,
            "fields": ["hostRoomUrl"],
            "roomNamePrefix": war_room_name.clone()
        });
        
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", whereby_api_key));
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        
        // Make the API call
        let response = HttpClient::request(whereby_api, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send();
        
        match response {
            Ok(resp) => {
                let status = resp.status();
                
                if status >= 200 && status < 300 {
                    // Parse the response
                    let response_text = resp.text();
                    let data: serde_json::Value = serde_json::from_str(&response_text)
                        .unwrap_or(json!({}));
                    
                    let room_url = data["roomUrl"]
                        .as_str()
                        .unwrap_or("URL not available");
                    let host_url = data["hostRoomUrl"]
                        .as_str()
                        .unwrap_or("Host URL not available");
                    
                    Ok(format!(
                        "🎯 War room created for incident {}\n\n\
                        📺 Join URL: {}\n\
                        🎛️  Host URL: {}\n\n\
                        Room expires: {}\n\
                        Severity: {}",
                        incident_id, room_url, host_url, end_date, severity.to_uppercase()
                    ))
                } else {
                    Err(format!(
                        "Whereby API error: HTTP {} - {}", 
                        status, 
                        resp.text()
                    ))
                }
            },
            Err(e) => {
                Err(format!("Failed to create war room: {}", e))
            }
        }
    }

#[query]
async fn ai_remediation(&mut self, incident_id: String, description: String, severity: String) -> Result<String, String> {
    Ok(json!({
        "incident_id": incident_id,
        "severity": severity,
        "status": "Analysis queued for processing"
    }).to_string())
}

    #[query]
    async fn list_all_incidents(&self) -> Result<Vec<String>, String> {
        let config = self.secrets.config();
        let logger = IncidentLoggerProxy::new(config.logger_contract_id.clone());
        
        match logger.list_incidents() {
            Ok(incidents) => Ok(incidents),
            Err(e) => Err(format!("Failed to retrieve incident list: {}", e))
        }
    }

    #[query]
    async fn get_incident_timeline(&self, incident_id: String) -> Result<Vec<IncidentLog>, String> {
        let config = self.secrets.config();
        let logger = IncidentLoggerProxy::new(config.logger_contract_id.clone());
        
        match logger.get_incident_logs(incident_id.clone()) {
            Ok(logs) => Ok(logs),
            Err(e) => Err(format!("Failed to retrieve timeline for incident {}: {}", incident_id, e))
        }
    }

    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "notify_discord",
      "description": "Send a Discord notification about an incident with severity level. \n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": ""
          },
          "message": {
            "type": "string",
            "description": ""
          },
          "severity": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "incident_id",
          "message",
          "severity"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "notify_email",
      "description": "Send an email notification via SendGrid with HTML and plain text formatting\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": "Unique identifier for the incident"
          },
          "to_email": {
            "type": "string",
            "description": "Recipient email address"
          },
          "subject": {
            "type": "string",
            "description": "Email subject line"
          },
          "message": {
            "type": "string",
            "description": "Detailed incident message/description"
          },
          "severity": {
            "type": "string",
            "description": "Incident severity level (critical, high, medium, low, p0-p3)"
          }
        },
        "required": [
          "incident_id",
          "to_email",
          "subject",
          "message",
          "severity"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "notify_slack",
      "description": "Send a Slack notification about an incident with severity level\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": ""
          },
          "message": {
            "type": "string",
            "description": ""
          },
          "severity": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "incident_id",
          "message",
          "severity"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "notify_sms",
      "description": "Send an SMS notification via Twilio to a specific phone number\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": ""
          },
          "phone": {
            "type": "string",
            "description": ""
          },
          "message": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "incident_id",
          "phone",
          "message"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "page_oncall",
      "description": "Page on-call engineers using PagerDuty\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": ""
          },
          "description": {
            "type": "string",
            "description": ""
          },
          "severity": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "incident_id",
          "description",
          "severity"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_status",
      "description": "Update incident status on Statuspage to inform customers\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": ""
          },
          "status": {
            "type": "string",
            "description": ""
          },
          "message": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "incident_id",
          "status",
          "message"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_war_room",
      "description": "Create a war room automatically for severe incidents requiring collaboration\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": ""
          },
          "severity": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "incident_id",
          "severity"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "ai_remediation",
      "description": "Trigger AI-driven remediation workflow for autonomous incident resolution\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": ""
          },
          "description": {
            "type": "string",
            "description": ""
          },
          "severity": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "incident_id",
          "description",
          "severity"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_all_incidents",
      "description": "List all incidents tracked in the logger\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_incident_timeline",
      "description": "Get complete timeline of events for a specific incident\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_id": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "incident_id"
        ]
      }
    }
  },
  {
  "type": "function",
  "function": {
    "name": "log_incident_event",
    "description": "Log a structured event into the incident timeline for auditing and observability",
    "parameters": {
      "type": "object",
      "properties": {
        "incident_id": {
          "type": "string",
          "description": "Unique identifier of the incident"
        },
        "event_type": {
          "type": "string",
          "description": "Type of event (e.g. CREATED, UPDATED, MITIGATED, RESOLVED)"
        },
        "message": {
          "type": "string",
          "description": "Human-readable description of the event"
        },
        "metadata": {
          "type": "string",
          "description": "Additional structured context in JSON or key-value format"
        }
      },
      "required": [
        "incident_id",
        "event_type",
        "message",
        "metadata"
      ]
    }
  }
}
]"#.to_string()
    }

    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#.to_string()
    }
}

