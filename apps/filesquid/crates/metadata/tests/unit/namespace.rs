//! Namespace registry and extension statements (MD-0.7; MD-S-012, MD-S-002,
//! MD-S-010).
//!
//! An owner, distribution or module registers an extension namespace
//! (prefix, IRI, optional term list); an owner may declare a custom
//! namespace of free keys. Registrations never redefine the built-in
//! namespaces, a prefix keeps one IRI, a term list rejects unknown terms and
//! an unregistered prefix is rejected by name. Extension statements live in
//! their own `metadata.md` map, round-trip through CSV, JSON and XMP, and are
//! never mapped onto `dcterms:`.

use filesquid_metadata::csv::{
    apply_csv, apply_csv_with, parse_csv_header_line, parse_csv_header_line_with,
    parse_csv_header_with, to_csv, CsvError,
};
use filesquid_metadata::json::{apply_json, apply_json_with, to_json, JsonError, JsonTarget};
use filesquid_metadata::markdown::{from_markdown, to_markdown};
use filesquid_metadata::namespace::{
    NamespaceKind, NamespaceRegistry, Registrant, Registration, RegistryError, MAX_REGISTRATIONS,
    MAX_TERMS,
};
use filesquid_metadata::record::{parse_curie, parse_curie_with, Record};
use filesquid_metadata::xmp::{from_xmp_with, to_xmp, to_xmp_with};
use filesquid_metadata::{MetadataError, MetadataNamespace};

/// The IRI the tests register for `rico`.
const RICO_IRI: &str = "https://www.ica.org/standards/RiC/ontology#";

/// The IRI of the owner's custom namespace.
const MINE_IRI: &str = "urn:uuid:0b5c4d1e-8a6f-4b21-9a0e-1f2d3c4b5a69";

/// Object identifier and digest a JSON sidecar binds.
const OBJECT_ID: &str = "urn:example:object-1";
const DIGEST: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// `terms` as owned strings.
fn terms(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

/// The `rico` extension with a term list, registered by a distribution.
fn rico() -> Registration {
    Registration::extension(
        "rico",
        RICO_IRI,
        Some(terms(&["title", "hasCreator", "date"])),
        Registrant::Distribution("arkrim".to_string()),
    )
}

/// The owner's custom namespace `mine`, free keys.
fn mine() -> Registration {
    Registration::custom("mine", MINE_IRI)
}

/// A registry holding `rico` and `mine`.
fn registry() -> NamespaceRegistry {
    let mut registry = NamespaceRegistry::default();
    registry.register(rico()).expect("rico");
    registry.register(mine()).expect("mine");
    registry
}

/// The namespace of extension prefix `prefix`.
fn ext(prefix: &str) -> MetadataNamespace {
    MetadataNamespace::Extension(prefix.to_string())
}

/// A record with `dcterms:title`, `rico:title`, `rico:hasCreator` and a custom key.
fn mixed_record() -> Record {
    let registry = registry();
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "title", "Harbour")
        .unwrap();
    record
        .set_registered(&registry, ext("rico"), "title", "Fonds 12, \"harbour\"")
        .unwrap();
    record
        .set_registered(&registry, ext("rico"), "hasCreator", "Port authority")
        .unwrap();
    record
        .set_registered(&registry, ext("mine"), "shelf_mark", "B-7: <left>")
        .unwrap();
    record
}

mod registration {
    use super::*;

    #[test]
    fn a_distribution_registers_an_extension_with_a_term_list() {
        let registry = registry();
        let found = registry.get("rico").expect("registered");
        assert_eq!(found.iri(), RICO_IRI);
        assert_eq!(found.kind(), NamespaceKind::Extension);
        assert_eq!(
            found.terms(),
            Some(&terms(&["title", "hasCreator", "date"])[..])
        );
        assert_eq!(
            found.registrant(),
            &Registrant::Distribution("arkrim".to_string())
        );
    }

    #[test]
    fn a_module_registers_an_extension_without_a_term_list() {
        let mut registry = NamespaceRegistry::default();
        let open = Registration::extension(
            "geo",
            "http://www.w3.org/2003/01/geo/wgs84_pos#",
            None,
            Registrant::Module("filesquid-media".to_string()),
        );
        assert_eq!(registry.register(open), Ok(true));
        assert_eq!(registry.check("geo", "anything"), Ok(()));
    }

    #[test]
    fn an_owner_declares_a_custom_namespace_of_free_keys() {
        let registry = registry();
        let found = registry.get("mine").unwrap();
        assert_eq!(found.kind(), NamespaceKind::Custom);
        assert_eq!(found.registrant(), &Registrant::Owner);
        assert_eq!(found.terms(), None);
        assert_eq!(registry.check("mine", "any_key.1"), Ok(()));
    }

    #[test]
    fn the_built_in_namespaces_cannot_be_redefined() {
        for prefix in ["dcterms", "dc", "schema", "exif"] {
            let mut registry = NamespaceRegistry::default();
            let again =
                Registration::extension(prefix, "https://example.org/x#", None, Registrant::Owner);
            assert_eq!(
                registry.register(again),
                Err(RegistryError::ReservedPrefix(prefix.to_string())),
                "{prefix}"
            );
            assert!(registry.registrations().is_empty());
        }
    }

    #[test]
    fn a_built_in_iri_cannot_be_registered_under_another_prefix() {
        for iri in [
            "http://purl.org/dc/terms/",
            "http://purl.org/dc/elements/1.1/",
            "https://schema.org/",
            "http://schema.org/",
            "http://ns.adobe.com/exif/1.0/",
        ] {
            let mut registry = NamespaceRegistry::default();
            let alias = Registration::extension("alias", iri, None, Registrant::Owner);
            assert_eq!(
                registry.register(alias),
                Err(RegistryError::ReservedIri(iri.to_string())),
                "{iri}"
            );
        }
    }

    #[test]
    fn the_record_format_keys_cannot_be_prefixes() {
        for prefix in [
            "schemaorg",
            "import",
            "namespaces",
            "original_name",
            "display_name",
        ] {
            let mut registry = NamespaceRegistry::default();
            let clash =
                Registration::extension(prefix, "https://example.org/x#", None, Registrant::Owner);
            assert_eq!(
                registry.register(clash),
                Err(RegistryError::ReservedPrefix(prefix.to_string())),
                "{prefix}"
            );
        }
    }

    #[test]
    fn re_registering_a_prefix_with_another_iri_is_rejected() {
        let mut registry = registry();
        let other =
            Registration::extension("rico", "https://example.org/rico#", None, Registrant::Owner);
        assert_eq!(
            registry.register(other),
            Err(RegistryError::PrefixTaken {
                prefix: "rico".to_string(),
                iri: RICO_IRI.to_string(),
            })
        );
        assert_eq!(registry.get("rico").unwrap().iri(), RICO_IRI);
    }

    #[test]
    fn an_identical_re_registration_changes_nothing() {
        let mut registry = registry();
        assert_eq!(registry.register(rico()), Ok(false));
        assert_eq!(registry.registrations().len(), 2);
    }

    #[test]
    fn the_same_iri_may_add_terms_but_never_remove_them() {
        let mut registry = registry();
        let more = Registration::extension(
            "rico",
            RICO_IRI,
            Some(terms(&["title", "hasCreator", "date", "scopeAndContent"])),
            Registrant::Distribution("arkrim".to_string()),
        );
        assert_eq!(registry.register(more), Ok(true));
        assert_eq!(registry.check("rico", "scopeAndContent"), Ok(()));
        let fewer =
            Registration::extension("rico", RICO_IRI, Some(terms(&["title"])), Registrant::Owner);
        assert_eq!(
            registry.register(fewer),
            Err(RegistryError::TermsRemoved("rico".to_string()))
        );
    }

    #[test]
    fn a_term_list_cannot_be_dropped_and_an_open_namespace_cannot_gain_one() {
        let mut registry = registry();
        let open = Registration::extension("rico", RICO_IRI, None, Registrant::Owner);
        assert_eq!(
            registry.register(open),
            Err(RegistryError::TermsRemoved("rico".to_string()))
        );
        assert_eq!(
            registry.check("rico", "colour"),
            Err(MetadataError::UnknownExtensionTerm {
                prefix: "rico".to_string(),
                term: "colour".to_string(),
            })
        );
        let mut base = NamespaceRegistry::default();
        base.register(rico()).unwrap();
        let text = base.to_markdown().replace(
            "\"terms\":[\"title\",\"hasCreator\",\"date\"]",
            "\"terms\":null",
        );
        let widened = NamespaceRegistry::from_markdown(&text).unwrap();
        assert_eq!(
            widened.check_extends(&base),
            Err(RegistryError::TermsRemoved("rico".to_string()))
        );
    }

    #[test]
    fn an_iri_belongs_to_one_prefix() {
        let mut registry = registry();
        let twin = Registration::extension("ric", RICO_IRI, None, Registrant::Owner);
        assert_eq!(
            registry.register(twin),
            Err(RegistryError::IriTaken {
                iri: RICO_IRI.to_string(),
                prefix: "rico".to_string(),
            })
        );
    }

    #[test]
    fn a_custom_namespace_cannot_become_an_extension() {
        let mut registry = registry();
        let changed = Registration::extension("mine", MINE_IRI, None, Registrant::Owner);
        assert_eq!(
            registry.register(changed),
            Err(RegistryError::KindChanged("mine".to_string()))
        );
    }

    #[test]
    fn prefixes_terms_and_iris_must_be_well_formed() {
        let mut registry = NamespaceRegistry::default();
        for prefix in [
            "",
            "Rico",
            "1ric",
            "ri co",
            "ri:co",
            "xmlns",
            "XMLfoo",
            &"a".repeat(65),
        ] {
            let bad =
                Registration::extension(prefix, "https://example.org/x#", None, Registrant::Owner);
            assert_eq!(
                registry.register(bad),
                Err(RegistryError::InvalidPrefix(prefix.to_string())),
                "{prefix:?}"
            );
        }
        for iri in [
            "",
            "no-scheme",
            "https://example.org/{x}",
            "https://example.org/a\\b",
            "https://example.org/a|b",
            "https://example.org/a^b",
            "https://example.org/a`b",
            "https://example.org/a b",
            &format!("https://{}", "a".repeat(2048)),
        ] {
            let bad = Registration::extension("ok", iri, None, Registrant::Owner);
            assert_eq!(
                registry.register(bad),
                Err(RegistryError::InvalidIri),
                "{iri:?}"
            );
        }
        for term in ["", "has creator", "a:b", "1st", "<x>"] {
            let bad = Registration::extension(
                "ok",
                "https://example.org/x#",
                Some(terms(&[term])),
                Registrant::Owner,
            );
            assert_eq!(
                registry.register(bad),
                Err(RegistryError::InvalidTerm(term.to_string())),
                "{term:?}"
            );
        }
        assert!(registry.registrations().is_empty());
    }

    #[test]
    fn duplicate_terms_in_a_list_are_refused() {
        let mut registry = NamespaceRegistry::default();
        let doubled = Registration::extension(
            "ok",
            "https://example.org/x#",
            Some(terms(&["title", "title"])),
            Registrant::Owner,
        );
        assert_eq!(
            registry.register(doubled),
            Err(RegistryError::DuplicateTerm("title".to_string()))
        );
    }

    #[test]
    fn registrations_and_term_lists_are_bounded() {
        let mut registry = NamespaceRegistry::default();
        for n in 0..MAX_REGISTRATIONS {
            let one = Registration::extension(
                &format!("ns{n}"),
                &format!("https://example.org/{n}#"),
                None,
                Registrant::Owner,
            );
            registry.register(one).unwrap();
        }
        let extra = Registration::extension(
            "extra",
            "https://example.org/extra#",
            None,
            Registrant::Owner,
        );
        assert_eq!(
            registry.register(extra),
            Err(RegistryError::TooManyRegistrations)
        );
        let names: Vec<String> = (0..=MAX_TERMS).map(|n| format!("t{n}")).collect();
        let big = Registration::extension(
            "big",
            "https://example.org/big#",
            Some(names),
            Registrant::Owner,
        );
        assert_eq!(
            NamespaceRegistry::default().register(big),
            Err(RegistryError::TooManyTerms("big".to_string()))
        );
    }
}

mod statements {
    use super::*;

    #[test]
    fn a_term_list_rejects_unknown_terms() {
        let registry = registry();
        let mut record = Record::default();
        assert_eq!(
            record.set_registered(&registry, ext("rico"), "colour", "red"),
            Err(MetadataError::UnknownExtensionTerm {
                prefix: "rico".to_string(),
                term: "colour".to_string(),
            })
        );
        assert!(record.statements.is_empty());
    }

    #[test]
    fn an_unregistered_prefix_is_rejected_by_name() {
        let registry = registry();
        let mut record = Record::default();
        let refused = record.set_registered(&registry, ext("lido"), "title", "x");
        assert_eq!(
            refused,
            Err(MetadataError::UnregisteredNamespace("lido".to_string()))
        );
        assert!(refused.unwrap_err().to_string().contains("lido"));
    }

    #[test]
    fn without_a_registry_an_extension_statement_is_unregistered() {
        let mut record = Record::default();
        assert_eq!(
            record.set(ext("rico"), "title", "x"),
            Err(MetadataError::UnregisteredNamespace("rico".to_string()))
        );
    }

    #[test]
    fn a_custom_key_must_be_a_valid_term_name() {
        let registry = registry();
        let mut record = Record::default();
        assert_eq!(
            record.set_registered(&registry, ext("mine"), "shelf mark", "x"),
            Err(MetadataError::UnknownExtensionTerm {
                prefix: "mine".to_string(),
                term: "shelf mark".to_string(),
            })
        );
    }

    #[test]
    fn the_registry_keeps_the_dcterms_register_and_the_dc_mapping() {
        let registry = registry();
        let mut record = Record::default();
        assert_eq!(
            record.set_registered(&registry, MetadataNamespace::Dcterms, "name", "x"),
            Err(MetadataError::UnknownDctermsTerm)
        );
        assert_eq!(
            parse_curie_with("dc:title", &registry),
            Ok((MetadataNamespace::Dcterms, "title"))
        );
        assert_eq!(
            parse_curie_with("dcterms:accrualMethod", &registry),
            Err(MetadataError::AccrualOnFile)
        );
    }

    #[test]
    fn a_curie_resolves_a_registered_prefix_and_names_an_unregistered_one() {
        let registry = registry();
        assert_eq!(
            parse_curie_with("rico:title", &registry),
            Ok((ext("rico"), "title"))
        );
        assert_eq!(
            parse_curie_with("rico:colour", &registry),
            Err(MetadataError::UnknownExtensionTerm {
                prefix: "rico".to_string(),
                term: "colour".to_string(),
            })
        );
        assert_eq!(
            parse_curie_with("lido:title", &registry),
            Err(MetadataError::UnregisteredNamespace("lido".to_string()))
        );
        assert_eq!(
            parse_curie("rico:title"),
            Err(MetadataError::UnregisteredNamespace("rico".to_string()))
        );
    }

    #[test]
    fn an_extension_statement_is_never_a_dcterms_statement() {
        let record = mixed_record();
        let title = record.get("rico:title").unwrap();
        assert_eq!(title.namespace, ext("rico"));
        assert_eq!(title.prefix(), "rico");
        assert_eq!(title.curie(), "rico:title");
        assert_eq!(record.get("dcterms:title").unwrap().value, "Harbour");
        assert_eq!(
            record
                .statements
                .iter()
                .filter(|s| s.namespace == MetadataNamespace::Dcterms)
                .count(),
            1
        );
    }
}

mod markdown {
    use super::*;

    #[test]
    fn each_extension_prefix_is_its_own_yaml_map() {
        let text = to_markdown(&mixed_record());
        assert!(text.contains("dcterms:\n  title: Harbour\n"), "{text}");
        assert!(
            text.contains(
                "rico:\n  title: \"Fonds 12, \\\"harbour\\\"\"\n  hasCreator: Port authority\n"
            ),
            "{text}"
        );
        assert!(
            text.contains("mine:\n  shelf_mark: \"B-7: <left>\"\n"),
            "{text}"
        );
    }

    #[test]
    fn extension_statements_round_trip_unchanged() {
        let record = mixed_record();
        assert_eq!(from_markdown(&to_markdown(&record)), record);
    }

    #[test]
    fn only_indented_lines_belong_to_a_map() {
        let record =
            from_markdown("---\nrico:\n  title: Kept\nstray: line\n  hasCreator: Lost\n---\n");
        assert_eq!(record.statements.len(), 1, "{record:?}");
        assert_eq!(record.get("rico:title").unwrap().value, "Kept");
    }

    #[test]
    fn a_blank_line_inside_a_map_keeps_the_map_open() {
        let record = from_markdown("---\nrico:\n  title: Kept\n\n  hasCreator: Also kept\n---\n");
        assert_eq!(record.get("rico:hasCreator").unwrap().value, "Also kept");
    }

    #[test]
    fn a_map_of_an_unregistered_prefix_is_kept_on_read() {
        let text = "---\ndcterms:\n  title: Harbour\nlido:\n  title: Kept\n---\n";
        let record = from_markdown(text);
        assert_eq!(record.get("lido:title").unwrap().value, "Kept");
        assert_eq!(to_markdown(&record), text);
    }
}

mod csv {
    use super::*;

    #[test]
    fn extension_statements_round_trip_through_csv() {
        let record = mixed_record();
        let text = to_csv(OBJECT_ID, &record);
        assert!(text.starts_with("id,dcterms:title,rico:title,rico:hasCreator,mine:shelf_mark\n"));
        let mut back = Record::default();
        apply_csv_with(&text, &registry(), &mut back).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn a_csv_header_line_resolves_registered_prefixes() {
        let headers =
            parse_csv_header_line_with("id,dcterms:title,rico:title", &registry()).unwrap();
        assert_eq!(headers[1].namespace, ext("rico"));
        assert_eq!(
            parse_csv_header_line("id,rico:title"),
            Err(CsvError::UnregisteredNamespace("rico".to_string()))
        );
    }

    #[test]
    fn a_csv_header_with_an_unregistered_prefix_is_rejected_by_name() {
        assert_eq!(
            parse_csv_header_with("lido:title", &registry()),
            Err(CsvError::UnregisteredNamespace("lido".to_string()))
        );
        let mut record = Record::default();
        assert_eq!(
            apply_csv("id,rico:title\nx,Fonds\n", &mut record),
            Err(CsvError::UnregisteredNamespace("rico".to_string()))
        );
    }

    #[test]
    fn a_csv_header_outside_a_term_list_is_rejected() {
        assert_eq!(
            parse_csv_header_with("rico:colour", &registry()),
            Err(CsvError::Registry(MetadataError::UnknownExtensionTerm {
                prefix: "rico".to_string(),
                term: "colour".to_string(),
            }))
        );
    }
}

mod json {
    use super::*;

    /// The binding of the test sidecar.
    fn target() -> JsonTarget<'static> {
        JsonTarget {
            object_id: OBJECT_ID,
            digest: DIGEST,
        }
    }

    #[test]
    fn extension_statements_round_trip_through_json() {
        let record = mixed_record();
        let text = to_json(OBJECT_ID, DIGEST, &record);
        assert!(text.contains("\"namespace\": \"rico\""), "{text}");
        let mut back = Record::default();
        apply_json_with(&text, &target(), &registry(), &mut back).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn json_import_applies_the_registry() {
        let text = to_json(OBJECT_ID, DIGEST, &mixed_record());
        let mut record = Record::default();
        assert_eq!(
            apply_json(&text, &target(), &mut record),
            Err(JsonError::UnregisteredNamespace("rico".to_string()))
        );
        let mut only_rico = NamespaceRegistry::default();
        only_rico.register(rico()).unwrap();
        assert_eq!(
            apply_json_with(&text, &target(), &only_rico, &mut record),
            Err(JsonError::UnregisteredNamespace("mine".to_string()))
        );
        assert!(record.statements.is_empty());
    }

    #[test]
    fn json_import_rejects_a_term_outside_the_list() {
        let text = format!(
            "{{\"objectId\":\"{OBJECT_ID}\",\"digestAlgorithm\":\"sha256\",\"digest\":\"{DIGEST}\",\
             \"statements\":[{{\"namespace\":\"rico\",\"term\":\"colour\",\"value\":\"red\"}}]}}"
        );
        let mut record = Record::default();
        assert_eq!(
            apply_json_with(&text, &target(), &registry(), &mut record),
            Err(JsonError::Register(MetadataError::UnknownExtensionTerm {
                prefix: "rico".to_string(),
                term: "colour".to_string(),
            }))
        );
    }
}

mod xmp {
    use super::*;

    #[test]
    fn extension_statements_round_trip_through_xmp() {
        let record = mixed_record();
        let text = to_xmp_with(&record, &registry()).unwrap();
        assert!(
            text.contains(&format!("xmlns:rico=\"{RICO_IRI}\"")),
            "{text}"
        );
        assert!(
            text.contains("<rico:hasCreator>Port authority</rico:hasCreator>"),
            "{text}"
        );
        assert!(
            text.contains("<mine:shelf_mark>B-7: &lt;left&gt;</mine:shelf_mark>"),
            "{text}"
        );
        let back = from_xmp_with(&text, &registry());
        let extension: Vec<_> = record
            .statements
            .iter()
            .filter(|s| s.namespace != MetadataNamespace::Exif)
            .cloned()
            .collect();
        assert_eq!(back.statements, extension);
    }

    #[test]
    fn xmp_export_rejects_an_unregistered_prefix_by_name() {
        let record = from_markdown("---\nlido:\n  title: Kept\n---\n");
        assert_eq!(
            to_xmp_with(&record, &registry()),
            Err(MetadataError::UnregisteredNamespace("lido".to_string()))
        );
        assert!(!to_xmp(&record).contains("lido"));
    }

    #[test]
    fn xmp_export_rejects_a_term_that_is_not_an_element_name() {
        let record = from_markdown("---\nmine:\n  shelf mark: B-7\n---\n");
        assert_eq!(
            to_xmp_with(&record, &registry()),
            Err(MetadataError::UnknownExtensionTerm {
                prefix: "mine".to_string(),
                term: "shelf mark".to_string(),
            })
        );
    }

    #[test]
    fn xmp_import_skips_a_packet_prefix_bound_to_two_iris() {
        let packet = format!(
            "<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
             <rdf:Description xmlns:r=\"{RICO_IRI}\"><r:title>Fonds 12</r:title>\
             <x:wrap xmlns:x=\"https://example.org/x#\" xmlns:r=\"https://example.org/other#\">\
             <r:hasCreator>Not RiC-O</r:hasCreator></x:wrap></rdf:Description></rdf:RDF>"
        );
        let record = from_xmp_with(&packet, &registry());
        assert!(record.get("rico:hasCreator").is_none(), "{record:?}");
        assert!(record.statements.is_empty(), "{record:?}");
    }

    #[test]
    fn xmp_import_reads_every_packet_prefix_of_a_registered_iri_in_one_pass() {
        let packet = format!(
            "<rdf:Description xmlns:a=\"{RICO_IRI}\" xmlns:b=\"{RICO_IRI}\" xmlns:m=\"{MINE_IRI}\">\
             <b:hasCreator>Port</b:hasCreator><a:title>Fonds</a:title><m:shelf>B-7</m:shelf>\
             </rdf:Description>"
        );
        let record = from_xmp_with(&packet, &registry());
        let curies: Vec<String> = record.statements.iter().map(|s| s.curie()).collect();
        assert_eq!(curies, ["rico:hasCreator", "rico:title", "mine:shelf"]);
    }

    #[test]
    fn xmp_import_reads_registered_properties_nested_in_another() {
        let packet = format!(
            "<rdf:Description xmlns:r=\"{RICO_IRI}\">\
             <r:date><r:title>Inner title</r:title><r:hasCreator>Inner creator</r:hasCreator></r:date>\
             </rdf:Description>"
        );
        let record = from_xmp_with(&packet, &registry());
        let curies: Vec<String> = record.statements.iter().map(|s| s.curie()).collect();
        assert_eq!(curies, ["rico:title", "rico:hasCreator"], "{record:?}");
    }

    #[test]
    fn xmp_import_reads_declarations_only_from_start_tags() {
        let packet = format!(
            "<rdf:Description xmlns:r=\"{RICO_IRI}\">\
             <!-- xmlns:r=\"https://example.org/comment#\" -->\
             <r:title>xmlns:r=\"https://example.org/value#\"</r:title>\
             </rdf:Description>"
        );
        let record = from_xmp_with(&packet, &registry());
        assert_eq!(
            record.get("rico:title").map(|s| s.value.as_str()),
            Some("xmlns:r=\"https://example.org/value#\"")
        );
    }

    #[test]
    fn xmp_import_reads_a_namespace_by_its_iri_under_any_packet_prefix() {
        let packet = format!(
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
             <rdf:Description xmlns:r='{RICO_IRI}' xmlns:other=\"https://example.org/other#\">\
             <r:title>Fonds 12</r:title><r:colour>red</r:colour><other:title>Ignored</other:title>\
             </rdf:Description></rdf:RDF></x:xmpmeta>"
        );
        let record = from_xmp_with(&packet, &registry());
        assert_eq!(record.statements.len(), 1);
        let title = record.get("rico:title").unwrap();
        assert_eq!(title.value, "Fonds 12");
        assert!(record.get("dcterms:title").is_none());
    }
}

mod persistence {
    use super::*;

    #[test]
    fn the_registry_round_trips_through_its_markdown_record() {
        let registry = registry();
        let text = registry.to_markdown();
        assert!(
            text.starts_with("---\nschema: filesquid.namespaces.v1\nnamespaces:\n  - {"),
            "{text}"
        );
        assert_eq!(NamespaceRegistry::from_markdown(&text), Ok(registry));
    }

    #[test]
    fn a_malformed_registry_record_is_refused() {
        for text in [
            "no front matter",
            "---\nschema: filesquid.other.v1\nnamespaces:\n---\n",
            "---\nschema: filesquid.namespaces.v1\nnamespaces:\n  - {\"prefix\":1}\n---\n",
            "---\nschema: filesquid.namespaces.v1\nnamespaces:\nstray: line\n---\n",
        ] {
            assert!(
                matches!(
                    NamespaceRegistry::from_markdown(text),
                    Err(RegistryError::Malformed(_))
                ),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_stored_registration_that_breaks_a_rule_is_refused_on_read() {
        let text = "---\nschema: filesquid.namespaces.v1\nnamespaces:\n  - {\"prefix\":\"dcterms\",\"iri\":\"https://example.org/x#\",\"kind\":\"extension\",\"registrant\":\"owner\",\"terms\":null}\n---\n";
        assert_eq!(
            NamespaceRegistry::from_markdown(text),
            Err(RegistryError::ReservedPrefix("dcterms".to_string()))
        );
    }

    #[test]
    fn a_registry_save_may_only_add() {
        let base = registry();
        let mut grown = base.clone();
        grown
            .register(Registration::extension(
                "geo",
                "https://example.org/geo#",
                None,
                Registrant::Owner,
            ))
            .unwrap();
        assert_eq!(grown.check_extends(&base), Ok(()));
        let mut shrunk = NamespaceRegistry::default();
        shrunk.register(rico()).unwrap();
        assert_eq!(
            shrunk.check_extends(&base),
            Err(RegistryError::RegistrationRemoved("mine".to_string()))
        );
    }
}
