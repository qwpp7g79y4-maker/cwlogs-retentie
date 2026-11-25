# Kwaliteit

Deze commit wijzigt alleen documentatie; de meting hieronder geldt voor codecommit 829cc6e.

Suite: 229 passed, 0 ignored, 0 failed.

Dit is de Nederlandse samenvatting van `QUALITY.md`. De Engelse tekst is leidend. Alleen gemeten waarden staan hier. Lokale runs worden nooit als CI-uitslag gemeld.

Gemeten op 2026-11-25 op een schone worktree van commit 829cc6e64e1d1d8d90820d5ef77b7b8757cc359d (`829cc6e`), de laatste commit die code of tests wijzigde; latere commits wijzigen alleen documentatie. rustc 1.99.0, cargo 1.99.0, Linux x86_64 (glibc). Elk resultaat in de tabel hieronder komt van deze ene run op één lokale machine. Versie 1.0.0 is niet gepubliceerd en heeft geen git-tag.

Alleen de vier commando's hieronder zijn in deze meting gedraaid. Wat verder in dit bestand staat en niet in die tabel (rode controles, tijd- en geheugencijfers, gesloten standaarduitvoer, bekende grenzen) is een eerder record en is hier niet herhaald.

CI (`.github/workflows/ci.yml`) is in deze meting niet gelezen of gestart: CI-uitslag niet uitgevoerd (not run).

## Uitgevoerde controles

| Commando | Afsluitcode |
| --- | ---: |
| `cargo fmt --check` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 |
| `cargo test --locked` (229 geslaagd) | 0 |
| `cargo test --release --locked` (229 geslaagd) | 0 |

Al het andere is in deze meting niet uitgevoerd: Rust 1.85, `cargo package`, `cargo deny`, `cargo audit`, `cargo about`, het uitgepakte `.crate`, fuzzen, mutatietests, Windows, macOS, musl en GitHub Actions.

## Testresultaat per target

| Target | Geslaagd | Mislukt | Genegeerd |
| --- | ---: | ---: | ---: |
| unittests `src/lib.rs` | 72 | 0 | 0 |
| `tests/claims.rs` | 27 | 0 | 0 |
| `tests/cli.rs` | 106 | 0 | 0 |
| `tests/docs_clean.rs` | 8 | 0 | 0 |
| `tests/handbook_help.rs` | 1 | 0 | 0 |
| `tests/readme.rs` | 15 | 0 | 0 |
| totaal | 229 | 0 | 0 |

## Claim → test

Elke claim van `README.md` ("Hoe het telt", C1 tot C24) met de test die hem bewijst, plus de regels over installatie, voorbeelden, versie en deze pagina. "Deels" betekent dat een test maar een deel van de claim bewijst; de reden staat bij dezelfde rij in `QUALITY.md`.

| # | Claim | Test | Status |
| --- | --- | --- | --- |
| C1 | Eén object is één groep; envelop, kale array en los object | `claim_c1_one_object_is_one_group`, `envelope_array_and_single_object` | getest |
| C2 | Opslag-USD is bytes / 1024^3 × tarief, exact berekend en één keer half weg van nul afgerond; totaal uit de exacte bedragen; 1.024 GB is 1 TB; 30 TB is 921,60 | `claim_c2_gibibyte_and_terabyte`, `page_thirty_terabytes_is_921_60`, `usd_rounds_half_away_from_zero_to_the_cent`, `many_small_groups_total_is_not_the_sum_of_rounded_rows`, `directory_total_is_the_sum_of_the_single_files`, `a_half_cent_from_a_30_digit_gb_rounds_up_like_the_exact_amount`, `a_total_that_is_an_exact_half_cent_rounds_up`, `a_saving_that_is_an_exact_half_cent_rounds_up`, `savings_match_the_integer_oracle_for_ages_retentions_and_horizons`, `a_half_cent_sum_of_many_denominators_rounds_once`, `distinct_ages_on_an_exact_half_cent_round_once_per_horizon` | getest |
| C3 | Standaardtarief 0,03 | `claim_c3_standard_rate` | getest |
| C4 | Infrequent Access 0,03 in het voorbeeld; tarief per klasse uit een prijsbestand | `claim_c4_infrequent_access_rate_and_override`, `prices_override_uses_the_class_rate` | getest |
| C5 | Delivery-tarief 0,03 | `claim_c5_delivery_rate` | getest |
| C6 | Bron, datum en regio worden getoond | `claim_c6_source_date_region`, `builtin_matches_the_pricing_page` | getest |
| C7 | De gratis 5 GB wordt niet afgetrokken | `claim_c7_free_tier_not_subtracted`, `six_gibibytes_do_not_use_the_five_gb_waiver` | getest |
| C8 | Ingestie en andere onderdelen worden niet geprijsd | `claim_c8_ingestion_not_priced` | getest |
| C9 | Ontbrekende of null-`retentionInDays` verloopt nooit | `claim_c9_missing_retention_never_expires`, `retention_rules` | getest |
| C10 | Horizon bij gelijkblijvende aanvoer en bespaarde USD | `claim_c10_steady_ingest_horizon`, `horizon_is_the_shorter_of_age_and_retention`, `zero_horizon_with_bytes_has_no_savings` | getest |
| C11 | Doelen 30, 90, 365; op of boven de horizon bespaart 0 | `claim_c11_targets_and_zero_at_horizon` | getest |
| C12 | `--as-of` is middernacht UTC, leeftijd in dagen, `age_days` met hoogstens 6 decimalen | `claim_c12_as_of_midnight_utc`, `half_day_age_is_exact`, `fractional_age_shows_at_most_six_decimals`, `as_of_accepts_only_digits`, `invalid_as_of_is_rejected` | getest |
| C13 | Zonder `--as-of` of `creationTime` vervallen de besparingen | `claim_c13_savings_omitted_without_as_of_or_creation`, `savings_need_an_as_of_date_and_a_creation_time` | getest |
| C14 | `--fail-stored-bytes` geeft 1 voor een groep zonder bewaartermijn; rapport blijft getoond; een overgeslagen lid is 2 | `claim_c14_fail_stored_bytes_exit_1`, `threshold_ignores_retained_groups_and_equal_bytes`, `exit_1_still_prints_the_report`, `fail_stored_bytes_refuses_when_a_member_was_skipped`, `fail_stored_bytes_refuses_when_a_non_utf8_member_was_skipped`, `a_file_name_that_starts_with_skipped_is_not_a_skipped_member` | getest |
| C15 | `--json` heeft geldstrings, bron, datum en een slotnewline | `claim_c15_json_money_strings`, `console_examples_match_the_binary` | getest |
| C15c | De gedocumenteerde `describe-log-groups`-vorm (`tests/fixtures/describe-log-groups-documented.json`: elk gedocumenteerd veld, minimale items, lege `inheritedProperties`, zowel `arn` als `logGroupArn`, namen met `_`, `.`, `#`, `/`, een pagina met `nextToken`, een lege pagina) wordt zonder fout gelezen en exact geprijsd (4,05 USD voor 135 GB) | `documented_shape_with_arn_and_log_group_arn_prices_exactly`, `documented_shape_text_output_has_every_name_and_the_same_total`, `documented_pages_with_a_next_token_an_empty_page_and_a_directory_are_read` | getest; geschreven uit de documentatie zonder netwerk (https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/API_DescribeLogGroups.html en https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/API_LogGroup.html), dus een later toegevoegd API-veld is niet gedekt |
| C16 | De adviesregel is tekst, noemt `put-retention-policy` en zegt niet uitgevoerd | `claim_c16_advice_is_not_executed` | getest |
| C17 | Geen AWS SDK, geen netwerk, `nextToken` niet gevolgd of getoond | `claim_c17_no_sdk_and_next_token_not_followed`, `next_token_is_not_stored` | deels getest |
| C18 | Dubbele namen zijn een fout zonder de ruwe JSON | `claim_c18_duplicate_names`, `duplicate_name_does_not_include_the_body` | getest |
| C19 | `storedBytes` is een niet-negatief geheel getal, foute waarden niet teruggegeven | `claim_c19_stored_bytes_integer`, `bad_name_and_bad_stored_bytes_are_not_echoed`, `large_integer_is_not_rounded` | getest |
| C20 | Naam van 1 tot 512 tekens uit het alfabet, foute naam niet teruggegeven | `claim_c20_log_group_name_alphabet`, `name_alphabet_bounds` | getest |
| C21 | Gzip aan de magische bytes; bytes na het eerste lid zijn een fout | `claim_c21_gzip_is_by_magic_bytes`, `gzip_bytes_after_the_first_member_are_an_error`, `corrupt_gzip_is_an_error` | getest |
| C22 | Account-id's niet getoond; een afwijkende regio is één waarschuwing | `claim_c22_arn_account_hidden`, `bad_class_is_not_echoed_and_arn_keeps_only_the_region`, `each_differing_arn_region_is_one_warning` | getest |
| C23 | Afgeronde rijen tellen niet op tot het totaal; geen extrapolatie per dag | `claim_c23_total_is_the_sum`, `many_small_groups_total_is_not_the_sum_of_rounded_rows` | deels getest |
| C24 | Fouten verbergen inhoud; stuur- en richtingtekens worden ge-escaped | `claim_c24_errors_hide_file_contents`, `control_chars_in_a_path_stay_on_one_line`, `escape_and_bell_in_a_path_stay_on_one_line`, `line_separator_and_bidi_override_are_not_raw`, `isolates_and_marks_are_escaped_in_labels_and_warnings`, `added_invisible_characters_are_escaped`, `bidi_and_line_separators_in_source_are_not_raw_in_output`, `clap_usage_errors_escape_raw_user_input`, `clap_errors_show_newline_cr_and_escape_as_unicode_escapes_on_one_line`, `symlink_warning_escapes_the_path`, `clap_error_with_a_non_utf8_argument_stays_on_one_line`, `newline_in_a_non_utf8_path_is_shown_as_a_unicode_escape`, `non_utf8_file_name_is_shown_as_escaped_bytes` | getest |
| L1 | Installatie vanaf een lokaal pad (Rust 1.85); de release-workflow wordt genoemd | `installation_uses_a_local_path` (`tests/readme.rs`) | getest |
| L2 | De snelstart leest `examples/describe-log-groups.json` en schrijft geen bestand, in beide README's | `quick_start_reads_the_sample_and_writes_nothing` | getest |
| L3 | De console-voorbeelden zijn wat de binary toont; het voorbeeldbestand en de map `examples/` | `console_examples_match_the_binary`, `example_file_prices_the_three_groups`, `the_examples_folder_documents_each_command_and_its_expected_output` | getest |
| L4 | README-koppen volgens de documentatiestandaard, grenzen vóór Installation, het gebruiksblok is de `--help`-tekst, `README.nl.md` heeft dezelfde console-blokken en claims, beide README's eindigen met Contact | `readme_sections_follow_the_documentation_standard`, `the_boundaries_come_before_installation`, `usage_block_is_the_help_text`, `the_dutch_readme_has_the_same_content`, `both_readmes_end_with_the_contact_section`, `every_how_it_counts_claim_has_a_test`, `limitations_name_behaviors_the_tests_cover` | getest |
| V1 | Een debugbuild toont de commit achter de versie; een releasebuild alleen de versie | `version_has_the_commit_in_debug_builds_only` | getest |
| Q1 | `QUALITY.md` en `QUALITY.nl.md` hebben per claim C1 tot C24 een rij met een bestaande test, een datum, een commit en een lijst van wat niet is uitgevoerd | `quality_claims_table_names_a_test_for_every_claim`, `quality_names_the_tested_commit_the_date_and_what_was_not_run` | getest |

De overige rijen (L5 tot L22, D1, S1, M1) staan in `QUALITY.md`.

## Niet uitgevoerd

- Niet uitgevoerd in deze meting: `cargo +1.85` (clippy en tests), `cargo package` en de testrun op het uitgepakte `.crate`, `cargo deny`, `cargo audit`, `cargo about`.
- GitHub Actions: de workflows zijn niet gestart (betaalprobleem), dus er is geen CI-resultaat op welk systeem dan ook; de commando's van de workflows zijn met de hand gedraaid (zie hierboven).
- De release-workflow `.github/workflows/release.yml`: nooit gedraaid, er is geen tag.
- Windows en macOS: er is geen test gedraaid, en de `cargo check`- en clippy-runs voor `x86_64-pc-windows-gnu` en `x86_64-apple-darwin` uit het record van 2026-11-19 zijn voor dit record niet herhaald (`build.rs` en de `--version`-constante zijn sindsdien nieuw).
- `cargo test --release` op Rust 1.85.
- De testsuite op musl.
- Een fuzz-campagne en een mutatierun.
- `git tag` en `cargo publish`: er is niets gepubliceerd.

## Bekende grenzen

- Deze pagina legt controles vast. Het is geen garantie en geen fuzz-rapport.
- De detectie van gesloten standaarduitvoer werkt alleen op Linux.
- `nextToken` wordt niet gevolgd, dus één antwoordpagina is niet het hele account.
- De ingebouwde tarieven zijn de us-east-1-voorbeelden van 2026-11-09 en zijn niet tegen de live pagina gecontroleerd.
- Het geheugen is niet door de bestandsgrootte begrensd: ongeveer 53 keer de bestandsgrootte voor een onbekend veld (zie "Beperkingen" in `README.nl.md`).
