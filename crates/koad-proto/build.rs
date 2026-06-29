fn main() -> Result<(), Box<dyn std::error::Error>> {
    let serde = "#[derive(serde::Serialize, serde::Deserialize)]";
    let default_on_missing = "#[serde(default)]";
    tonic_build::configure()
        .type_attribute("koad.cass.v1.MemoryMetadata", serde)
        .type_attribute("koad.cass.v1.TokenEstimate", serde)
        .type_attribute("koad.cass.v1.PromptBudgetHints", serde)
        .type_attribute("koad.cass.v1.RetrievalMetadata", serde)
        .type_attribute("koad.cass.v1.ProvenanceMetadata", serde)
        .type_attribute("koad.cass.v1.PrivacyMetadata", serde)
        .field_attribute("koad.cass.v1.MemoryMetadata", default_on_missing)
        .field_attribute("koad.cass.v1.TokenEstimate", default_on_missing)
        .field_attribute("koad.cass.v1.PromptBudgetHints", default_on_missing)
        .field_attribute("koad.cass.v1.RetrievalMetadata", default_on_missing)
        .field_attribute("koad.cass.v1.ProvenanceMetadata", default_on_missing)
        .field_attribute("koad.cass.v1.PrivacyMetadata", default_on_missing)
        .compile_protos(
            &[
                "../../proto/skill.proto",
                "../../proto/citadel.proto",
                "../../proto/cass.proto",
            ],
            &["../../proto"],
        )?;
    Ok(())
}
