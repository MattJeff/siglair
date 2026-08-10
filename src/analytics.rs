//! Validation des événements analytics produits par la landing et l'application.
//!
//! Le navigateur n'est pas une source de confiance. Il choisit un événement dans une liste
//! fermée et fournit quelques dimensions bornées ; l'identité connectée, le pays, le type
//! d'appareil et le navigateur sont enrichis côté serveur dans `routes::events`.

use std::{collections::BTreeMap, str::FromStr};

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::Value;
use url::Url;
use uuid::Uuid;

pub const MAX_BATCH_EVENTS: usize = 20;
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_PROPERTIES: usize = 16;
const MAX_PROPERTIES_BYTES: usize = 4 * 1024;
const MAX_STRING_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventName {
    PageViewed,
    LandingViewed,
    PricingViewed,
    HeroCtaClicked,
    NavigationClicked,
    FaqOpened,
    UrlInputFocused,
    UrlEntered,
    UrlSubmitted,
    GenerationStarted,
    GenerationCompleted,
    GenerationFailed,
    PreviewViewed,
    ClaimClicked,
    SignupStarted,
    SignupCompleted,
    LoginStarted,
    LoginCompleted,
    EditorOpened,
    EditorSessionEnded,
    BlockAdded,
    BlockDeleted,
    BlockMoved,
    AssetUploaded,
    LogoChanged,
    ProfilePictureChanged,
    FontChanged,
    ColorsChanged,
    LayoutChanged,
    TemplateSelected,
    AnimationSelected,
    CtaAdded,
    SocialIconAdded,
    FieldAdded,
    FieldRemoved,
    UndoClicked,
    RedoClicked,
    PreviewOpened,
    MobilePreviewOpened,
    DesktopPreviewOpened,
    SignatureSaved,
    PublishStarted,
    PublishCompleted,
    PublishFailed,
    ExportStarted,
    ExportCompleted,
    InstallStarted,
    InstallCompleted,
    UpgradeClicked,
    CheckoutStarted,
    CampaignCreated,
    CampaignScheduled,
    CampaignPublished,
    AnalyticsViewed,
    SettingsUpdated,
    BillingViewed,
    AiPromptSubmitted,
    AiEditCompleted,
    AiEditFailed,
    ReferralArrived,
    OutboundLinkClicked,
    GenerationRegenerated,
    SignupFailed,
    GeneratedSignatureClaimed,
    EditorAction,
    EditorPreviewOpened,
    SignaturePublished,
    InstallInstructionsViewed,
    InstallStepCompleted,
    InstallFailed,
    InstallAbandoned,
    TestEmailStarted,
    TestEmailCompleted,
    SignatureVerified,
    UpgradePromptViewed,
    CheckoutReturned,
    TeamInviteSent,
}

impl EventName {
    pub const ALL: [Self; 77] = [
        Self::PageViewed,
        Self::LandingViewed,
        Self::PricingViewed,
        Self::HeroCtaClicked,
        Self::NavigationClicked,
        Self::FaqOpened,
        Self::UrlInputFocused,
        Self::UrlEntered,
        Self::UrlSubmitted,
        Self::GenerationStarted,
        Self::GenerationCompleted,
        Self::GenerationFailed,
        Self::PreviewViewed,
        Self::ClaimClicked,
        Self::SignupStarted,
        Self::SignupCompleted,
        Self::LoginStarted,
        Self::LoginCompleted,
        Self::EditorOpened,
        Self::EditorSessionEnded,
        Self::BlockAdded,
        Self::BlockDeleted,
        Self::BlockMoved,
        Self::AssetUploaded,
        Self::LogoChanged,
        Self::ProfilePictureChanged,
        Self::FontChanged,
        Self::ColorsChanged,
        Self::LayoutChanged,
        Self::TemplateSelected,
        Self::AnimationSelected,
        Self::CtaAdded,
        Self::SocialIconAdded,
        Self::FieldAdded,
        Self::FieldRemoved,
        Self::UndoClicked,
        Self::RedoClicked,
        Self::PreviewOpened,
        Self::MobilePreviewOpened,
        Self::DesktopPreviewOpened,
        Self::SignatureSaved,
        Self::PublishStarted,
        Self::PublishCompleted,
        Self::PublishFailed,
        Self::ExportStarted,
        Self::ExportCompleted,
        Self::InstallStarted,
        Self::InstallCompleted,
        Self::UpgradeClicked,
        Self::CheckoutStarted,
        Self::CampaignCreated,
        Self::CampaignScheduled,
        Self::CampaignPublished,
        Self::AnalyticsViewed,
        Self::SettingsUpdated,
        Self::BillingViewed,
        Self::AiPromptSubmitted,
        Self::AiEditCompleted,
        Self::AiEditFailed,
        Self::ReferralArrived,
        Self::OutboundLinkClicked,
        Self::GenerationRegenerated,
        Self::SignupFailed,
        Self::GeneratedSignatureClaimed,
        Self::EditorAction,
        Self::EditorPreviewOpened,
        Self::SignaturePublished,
        Self::InstallInstructionsViewed,
        Self::InstallStepCompleted,
        Self::InstallFailed,
        Self::InstallAbandoned,
        Self::TestEmailStarted,
        Self::TestEmailCompleted,
        Self::SignatureVerified,
        Self::UpgradePromptViewed,
        Self::CheckoutReturned,
        Self::TeamInviteSent,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PageViewed => "page_viewed",
            Self::LandingViewed => "landing_viewed",
            Self::PricingViewed => "pricing_viewed",
            Self::HeroCtaClicked => "hero_cta_clicked",
            Self::NavigationClicked => "navigation_clicked",
            Self::FaqOpened => "faq_opened",
            Self::UrlInputFocused => "url_input_focused",
            Self::UrlEntered => "url_entered",
            Self::UrlSubmitted => "url_submitted",
            Self::GenerationStarted => "generation_started",
            Self::GenerationCompleted => "generation_completed",
            Self::GenerationFailed => "generation_failed",
            Self::PreviewViewed => "preview_viewed",
            Self::ClaimClicked => "claim_clicked",
            Self::SignupStarted => "signup_started",
            Self::SignupCompleted => "signup_completed",
            Self::LoginStarted => "login_started",
            Self::LoginCompleted => "login_completed",
            Self::EditorOpened => "editor_opened",
            Self::EditorSessionEnded => "editor_session_ended",
            Self::BlockAdded => "block_added",
            Self::BlockDeleted => "block_deleted",
            Self::BlockMoved => "block_moved",
            Self::AssetUploaded => "asset_uploaded",
            Self::LogoChanged => "logo_changed",
            Self::ProfilePictureChanged => "profile_picture_changed",
            Self::FontChanged => "font_changed",
            Self::ColorsChanged => "colors_changed",
            Self::LayoutChanged => "layout_changed",
            Self::TemplateSelected => "template_selected",
            Self::AnimationSelected => "animation_selected",
            Self::CtaAdded => "cta_added",
            Self::SocialIconAdded => "social_icon_added",
            Self::FieldAdded => "field_added",
            Self::FieldRemoved => "field_removed",
            Self::UndoClicked => "undo_clicked",
            Self::RedoClicked => "redo_clicked",
            Self::PreviewOpened => "preview_opened",
            Self::MobilePreviewOpened => "mobile_preview_opened",
            Self::DesktopPreviewOpened => "desktop_preview_opened",
            Self::SignatureSaved => "signature_saved",
            Self::PublishStarted => "publish_started",
            Self::PublishCompleted => "publish_completed",
            Self::PublishFailed => "publish_failed",
            Self::ExportStarted => "export_started",
            Self::ExportCompleted => "export_completed",
            Self::InstallStarted => "install_started",
            Self::InstallCompleted => "install_completed",
            Self::UpgradeClicked => "upgrade_clicked",
            Self::CheckoutStarted => "checkout_started",
            Self::CampaignCreated => "campaign_created",
            Self::CampaignScheduled => "campaign_scheduled",
            Self::CampaignPublished => "campaign_published",
            Self::AnalyticsViewed => "analytics_viewed",
            Self::SettingsUpdated => "settings_updated",
            Self::BillingViewed => "billing_viewed",
            Self::AiPromptSubmitted => "ai_prompt_submitted",
            Self::AiEditCompleted => "ai_edit_completed",
            Self::AiEditFailed => "ai_edit_failed",
            Self::ReferralArrived => "referral_arrived",
            Self::OutboundLinkClicked => "outbound_link_clicked",
            Self::GenerationRegenerated => "generation_regenerated",
            Self::SignupFailed => "signup_failed",
            Self::GeneratedSignatureClaimed => "generated_signature_claimed",
            Self::EditorAction => "editor_action",
            Self::EditorPreviewOpened => "editor_preview_opened",
            Self::SignaturePublished => "signature_published",
            Self::InstallInstructionsViewed => "install_instructions_viewed",
            Self::InstallStepCompleted => "install_step_completed",
            Self::InstallFailed => "install_failed",
            Self::InstallAbandoned => "install_abandoned",
            Self::TestEmailStarted => "test_email_started",
            Self::TestEmailCompleted => "test_email_completed",
            Self::SignatureVerified => "signature_verified",
            Self::UpgradePromptViewed => "upgrade_prompt_viewed",
            Self::CheckoutReturned => "checkout_returned",
            Self::TeamInviteSent => "team_invite_sent",
        }
    }
}

impl FromStr for EventName {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|event| event.as_str() == value)
            .ok_or_else(|| ValidationError::new("Nom d'événement inconnu."))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestRequest {
    pub events: Vec<EventInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    pub event_id: Uuid,
    pub name: String,
    pub visitor_id: Uuid,
    pub session_id: Uuid,
    #[serde(default)]
    pub properties: BTreeMap<String, Value>,
    #[serde(default)]
    pub context: EventContext,
    pub occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventContext {
    pub page_path: Option<String>,
    pub referrer: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_content: Option<String>,
    pub utm_term: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug)]
pub struct ValidatedEvent {
    pub event_id: Uuid,
    pub name: EventName,
    pub visitor_id: Uuid,
    pub session_id: Uuid,
    pub properties: Value,
    pub page_path: Option<String>,
    pub referrer_host: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_content: Option<String>,
    pub utm_term: Option<String>,
    pub language: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ValidatedBatch {
    pub events: Vec<ValidatedEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError(String);

impl ValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl ValidatedBatch {
    pub fn validate(request: IngestRequest, now: DateTime<Utc>) -> Result<Self, ValidationError> {
        if request.events.is_empty() || request.events.len() > MAX_BATCH_EVENTS {
            return Err(ValidationError::new(format!(
                "Un lot doit contenir entre 1 et {MAX_BATCH_EVENTS} événements."
            )));
        }

        let visitor_id = request.events[0].visitor_id;
        let session_id = request.events[0].session_id;
        if visitor_id.is_nil() || session_id.is_nil() {
            return Err(ValidationError::new(
                "Les identifiants visiteur et session doivent être des UUID non nuls.",
            ));
        }

        let mut event_ids = std::collections::HashSet::with_capacity(request.events.len());
        let mut events = Vec::with_capacity(request.events.len());
        for input in request.events {
            if input.visitor_id != visitor_id || input.session_id != session_id {
                return Err(ValidationError::new(
                    "Tous les événements d'un lot doivent appartenir au même visiteur et à la même session.",
                ));
            }
            if input.event_id.is_nil() || !event_ids.insert(input.event_id) {
                return Err(ValidationError::new(
                    "Chaque événement du lot doit avoir un event_id unique et non nul.",
                ));
            }
            events.push(validate_event(input, now)?);
        }
        Ok(Self { events })
    }
}

fn validate_event(
    input: EventInput,
    now: DateTime<Utc>,
) -> Result<ValidatedEvent, ValidationError> {
    let name = EventName::from_str(&input.name)?;
    let occurred_at = input.occurred_at.unwrap_or(now);
    if occurred_at < now - Duration::days(7) || occurred_at > now + Duration::minutes(5) {
        return Err(ValidationError::new(
            "occurred_at doit être compris entre les 7 derniers jours et 5 minutes dans le futur.",
        ));
    }

    let properties = validate_properties(input.properties)?;
    let page_path = input
        .context
        .page_path
        .map(validate_page_path)
        .transpose()?;
    let referrer_host = input
        .context
        .referrer
        .as_deref()
        .map(referrer_host)
        .transpose()?;

    Ok(ValidatedEvent {
        event_id: input.event_id,
        name,
        visitor_id: input.visitor_id,
        session_id: input.session_id,
        properties,
        page_path,
        referrer_host,
        utm_source: optional_dimension(input.context.utm_source, 64, "utm_source")?,
        utm_medium: optional_dimension(input.context.utm_medium, 64, "utm_medium")?,
        utm_campaign: optional_dimension(input.context.utm_campaign, 128, "utm_campaign")?,
        utm_content: optional_dimension(input.context.utm_content, 128, "utm_content")?,
        utm_term: optional_dimension(input.context.utm_term, 128, "utm_term")?,
        language: optional_language(input.context.language)?,
        occurred_at,
    })
}

fn validate_properties(properties: BTreeMap<String, Value>) -> Result<Value, ValidationError> {
    if properties.len() > MAX_PROPERTIES {
        return Err(ValidationError::new(format!(
            "Les propriétés sont limitées à {MAX_PROPERTIES} clés."
        )));
    }

    for (key, value) in &properties {
        let kind = property_kind(key)
            .ok_or_else(|| ValidationError::new(format!("Propriété non autorisée : {key}.")))?;
        match (kind, value) {
            (PropertyKind::Boolean, Value::Bool(_)) => {}
            (PropertyKind::Number, Value::Number(number))
                if number.as_f64().is_some_and(|number| {
                    number.is_finite() && (0.0..=1_000_000_000.0).contains(&number)
                }) => {}
            (PropertyKind::Uuid, Value::String(value)) if Uuid::parse_str(value).is_ok() => {}
            (PropertyKind::Host, Value::String(value)) if valid_host(value) => {}
            (PropertyKind::Dimension, Value::String(value))
                if valid_dimension(value, MAX_STRING_BYTES) => {}
            _ => {
                return Err(ValidationError::new(format!(
                    "Valeur invalide pour la propriété {key}."
                )))
            }
        }
    }

    let value = serde_json::to_value(properties)
        .map_err(|_| ValidationError::new("Propriétés JSON invalides."))?;
    if serde_json::to_vec(&value)
        .map_err(|_| ValidationError::new("Propriétés JSON invalides."))?
        .len()
        > MAX_PROPERTIES_BYTES
    {
        return Err(ValidationError::new(
            "Les propriétés dépassent la taille maximale de 4 Ko.",
        ));
    }
    Ok(value)
}

#[derive(Clone, Copy)]
enum PropertyKind {
    Boolean,
    Number,
    Uuid,
    Host,
    Dimension,
}

fn property_kind(key: &str) -> Option<PropertyKind> {
    use PropertyKind::*;
    Some(match key {
        "zero_edit" | "ai_used" | "is_returning" | "authenticated" | "had_protocol"
        | "logo_detected" | "colors_detected" | "contacts_detected" | "recoverable"
        | "handoff_preserved" | "successful" => Boolean,
        "duration_ms" | "latency_ms" | "edit_count" | "block_count" | "generation_count"
        | "member_count" | "seat_count" | "attempt" | "viral_depth" | "changed_elements"
        | "step" | "total_steps" | "last_step" => Number,
        "signature_id" | "campaign_id" | "generation_id" => Uuid,
        "site_host" | "destination_host" | "target_host" => Host,
        "area"
        | "placement"
        | "source"
        | "medium"
        | "method"
        | "provider"
        | "plan"
        | "billing_interval"
        | "template_id"
        | "animation_id"
        | "element_type"
        | "field_type"
        | "preview_client"
        | "export_format"
        | "install_client"
        | "failure_code"
        | "result"
        | "mode"
        | "campaign_type"
        | "ai_action"
        | "asset_kind"
        | "cta_kind"
        | "layout_id"
        | "device_mode"
        | "page"
        | "referral_id"
        | "variant"
        | "input_method"
        | "origin"
        | "error_code"
        | "reason"
        | "action"
        | "previous_template_id"
        | "client"
        | "intent"
        | "prompt_length_bucket"
        | "trigger"
        | "current_plan"
        | "target_plan"
        | "billing_period"
        | "status"
        | "role" => Dimension,
        _ => return None,
    })
}

fn validate_page_path(value: String) -> Result<String, ValidationError> {
    if value.starts_with('/')
        && value.len() <= 256
        && !value.contains(['?', '#', '@', '\n', '\r'])
        && !value.chars().any(char::is_control)
    {
        Ok(value)
    } else {
        Err(ValidationError::new(
            "page_path doit être un chemin sans query string ni fragment.",
        ))
    }
}

fn referrer_host(value: &str) -> Result<String, ValidationError> {
    let url = Url::parse(value)
        .map_err(|_| ValidationError::new("Le referrer doit être une URL HTTP valide."))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ValidationError::new(
            "Le referrer doit être une URL HTTP valide.",
        ));
    }
    let host = url
        .host_str()
        .filter(|host| valid_host(host))
        .ok_or_else(|| ValidationError::new("Le referrer doit contenir un domaine valide."))?;
    Ok(host.to_ascii_lowercase())
}

fn optional_dimension(
    value: Option<String>,
    max: usize,
    field: &str,
) -> Result<Option<String>, ValidationError> {
    value
        .map(|value| {
            if valid_dimension(&value, max) {
                Ok(value)
            } else {
                Err(ValidationError::new(format!(
                    "Dimension {field} invalide ou trop longue."
                )))
            }
        })
        .transpose()
}

fn optional_language(value: Option<String>) -> Result<Option<String>, ValidationError> {
    value
        .map(|value| {
            let valid = !value.is_empty()
                && value.len() <= 16
                && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            valid
                .then_some(value)
                .ok_or_else(|| ValidationError::new("Langue invalide."))
        })
        .transpose()
}

fn valid_dimension(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value.contains('@')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '/'))
}

fn valid_host(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && !value.contains(['/', '@', ':'])
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

pub fn browser_family(user_agent: Option<&str>) -> &'static str {
    let ua = user_agent.unwrap_or_default().to_ascii_lowercase();
    if ua.contains("bot") || ua.contains("crawler") || ua.contains("spider") {
        "bot"
    } else if ua.contains("edg/") || ua.contains("edge/") {
        "edge"
    } else if ua.contains("firefox/") || ua.contains("fxios/") {
        "firefox"
    } else if ua.contains("chrome/") || ua.contains("crios/") {
        "chrome"
    } else if ua.contains("safari/") {
        "safari"
    } else {
        "other"
    }
}

pub fn device_type(user_agent: Option<&str>) -> &'static str {
    let ua = user_agent.unwrap_or_default().to_ascii_lowercase();
    if ua.contains("bot") || ua.contains("crawler") || ua.contains("spider") {
        "bot"
    } else if ua.contains("ipad") || ua.contains("tablet") {
        "tablet"
    } else if ua.contains("mobile") || ua.contains("iphone") || ua.contains("android") {
        "mobile"
    } else if ua.is_empty() {
        "other"
    } else {
        "desktop"
    }
}

pub fn country_code(value: Option<&str>) -> Option<String> {
    let value = value?.trim().to_ascii_uppercase();
    (value.len() == 2 && value.chars().all(|c| c.is_ascii_alphabetic())).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MIGRATION: &str = include_str!("../migrations/0008_product_analytics.sql");

    fn event(name: &str) -> EventInput {
        EventInput {
            event_id: Uuid::new_v4(),
            name: name.into(),
            visitor_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            properties: BTreeMap::new(),
            context: EventContext::default(),
            occurred_at: None,
        }
    }

    fn request_with(event: EventInput) -> IngestRequest {
        IngestRequest {
            events: vec![event],
        }
    }

    #[test]
    fn allowlist_rust_et_check_sql_restent_alignes() {
        for event in EventName::ALL {
            assert!(
                MIGRATION.contains(&format!("'{}'", event.as_str())),
                "{} manque dans la migration",
                event.as_str()
            );
            assert_eq!(EventName::from_str(event.as_str()), Ok(event));
        }
        assert!(EventName::from_str("arbitrary_tracking").is_err());
    }

    #[test]
    fn lot_borne_et_identite_coherente() {
        let now = Utc::now();
        assert!(ValidatedBatch::validate(IngestRequest { events: vec![] }, now).is_err());

        let mut events = Vec::new();
        for _ in 0..=MAX_BATCH_EVENTS {
            events.push(event("page_viewed"));
        }
        assert!(ValidatedBatch::validate(IngestRequest { events }, now).is_err());

        let first = event("page_viewed");
        let mut second = event("hero_cta_clicked");
        second.visitor_id = first.visitor_id;
        second.session_id = first.session_id;
        second.event_id = first.event_id;
        assert!(ValidatedBatch::validate(
            IngestRequest {
                events: vec![first, second]
            },
            now
        )
        .unwrap_err()
        .message()
        .contains("event_id"));
    }

    #[test]
    fn refuse_proprietes_libres_et_pii() {
        let now = Utc::now();
        let mut with_email = event("signup_started");
        with_email
            .properties
            .insert("email".into(), json!("personne@example.com"));
        assert!(ValidatedBatch::validate(request_with(with_email), now).is_err());

        let mut free_text = event("ai_prompt_submitted");
        free_text
            .properties
            .insert("ai_action".into(), json!("contact@example.com"));
        assert!(ValidatedBatch::validate(request_with(free_text), now).is_err());

        let mut nested = event("editor_opened");
        nested
            .properties
            .insert("mode".into(), json!({"raw": "secret"}));
        assert!(ValidatedBatch::validate(request_with(nested), now).is_err());
    }

    #[test]
    fn ne_garde_que_le_domaine_du_referrer() {
        let now = Utc::now();
        let mut input = event("landing_viewed");
        input.context.referrer =
            Some("https://www.google.com/search?q=nom%40example.com#secret".into());
        let out = ValidatedBatch::validate(request_with(input), now).unwrap();
        assert_eq!(
            out.events[0].referrer_host.as_deref(),
            Some("www.google.com")
        );
    }

    #[test]
    fn refuse_url_complete_et_horodatage_aberrant() {
        let now = Utc::now();
        let mut input = event("page_viewed");
        input.context.page_path = Some("/app?token=secret".into());
        assert!(ValidatedBatch::validate(request_with(input), now).is_err());

        let mut old = event("page_viewed");
        old.occurred_at = Some(now - Duration::days(8));
        assert!(ValidatedBatch::validate(request_with(old), now).is_err());
    }

    #[test]
    fn reduit_user_agent_et_pays_sans_conserver_le_brut() {
        let iphone = "Mozilla/5.0 (iPhone) AppleWebKit/605.1 Version/17.0 Mobile Safari/604.1";
        assert_eq!(browser_family(Some(iphone)), "safari");
        assert_eq!(device_type(Some(iphone)), "mobile");
        assert_eq!(browser_family(Some("ExampleBot/1.0")), "bot");
        assert_eq!(country_code(Some("fr")).as_deref(), Some("FR"));
        assert_eq!(country_code(Some("France")), None);
    }
}
