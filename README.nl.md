# cwlogs-retentie

[![crates.io](https://img.shields.io/crates/v/cwlogs-retentie.svg)](https://crates.io/crates/cwlogs-retentie) [![downloads](https://img.shields.io/crates/d/cwlogs-retentie.svg)](https://crates.io/crates/cwlogs-retentie) [![CI](https://github.com/qwpp7g79y4-maker/cwlogs-retentie/actions/workflows/ci.yml/badge.svg)](https://github.com/qwpp7g79y4-maker/cwlogs-retentie/actions/workflows/ci.yml) [![PEPAI checked](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fqwpp7g79y4-maker%2Fcwlogs-retentie%2Fmain%2F.github%2Fbadge.json)](QUALITY.md) [![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://github.com/qwpp7g79y4-maker/cwlogs-retentie/blob/main/Cargo.toml) [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/qwpp7g79y4-maker/cwlogs-retentie#license)

Schat wat de opgeslagen bytes van je CloudWatch Logs-loggroepen per maand kosten, vind de groepen die nooit verlopen, en zie wat een bewaartermijn van 30, 90 of 365 dagen zou besparen. Bedoeld voor iedereen met een export van `aws logs describe-log-groups` die de cijfers wil zonder de export ergens heen te sturen. Het resultaat is een tabel met USD-bedragen die je met de hand kunt narekenen (zie "Hoe het telt").

De tests en de gemeten getallen staan in `QUALITY.md`.

Een `describe-log-groups`-export bevat al de opgeslagen bytes en de bewaartermijn van elke loggroep. cwlogs-retentie prijst die opslag op je eigen machine en noemt de groepen die nooit verlopen. Het roept AWS niet aan en heeft geen account nodig.

Wat het wel en niet doet:

- Het leest de bestanden die je noemt (JSON, gzip, een map of standaardinvoer) en het prijsbestand als je er een meegeeft. Het gebruikt geen netwerk: er is geen AWS SDK en een `nextToken` wordt niet gevolgd.
- Het schrijft niets, behalve het ene bestand dat je met `--output` noemt. Het voert nooit een commando uit: de adviesregels tonen `aws logs put-retention-policy ...` en zeggen dat ze niet zijn uitgevoerd.
- Het heeft geen account, token of betaalde dienst nodig.
- Elk bedrag is een schatting uit lijstprijzen, niet je AWS-factuur; de uitvoer zegt dat. De ingebouwde prijzen zijn van 2026-11-09 en de tool waarschuwt als de prijsgegevens ouder zijn dan 180 dagen.

## Installatie

Met Rust 1.85 of nieuwer, vanuit een kloon van deze repository:

```sh
cargo install --locked --path .
cwlogs-retentie --help
```

Deze crate is niet gepubliceerd, dus er is geen `cargo install cwlogs-retentie` vanaf crates.io. Binaire archieven voor Linux (glibc en musl), Windows en macOS worden door `.github/workflows/release.yml` gebouwd zodra een `v*`-tag wordt gepusht; er bestaat nog geen tag, dus er is nog geen archief om te downloaden. Elk archief zou de binary, de man-pagina, de README's, QUALITY, de licenties en de lijst met derde-partij-licenties bevatten.

Ondersteunde systemen: Linux x86_64 is gebouwd en getest. De code compileert voor Windows en macOS, maar de tests zijn daar niet gedraaid (zie `QUALITY.nl.md`). Alleen op Unix werken de SIGINT-afhandeling en de controle op gesloten standaarduitvoer.

`cargo test --locked` voert 229 tests uit (72 unit, 27 claims, 106 CLI, 8 docs, 1 handbook, 15 README); zie `QUALITY.nl.md`.

## Snelstart

`examples/describe-log-groups.json` is één verzonnen pagina van `aws logs describe-log-groups`. Draai dit vanuit de hoofdmap van de repository; het leest alleen het bestand en schrijft niets:

```console
$ cwlogs-retentie --quiet --as-of 2026-11-09 examples/describe-log-groups.json
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

log_group             class              retention  age_days  stored_bytes   gb  estimated_usd_month  save_30_usd  save_90_usd  save_365_usd
/aws/ecs/worker       INFREQUENT_ACCESS       none        10   10737418240   10                 0.30         0.00         0.00          0.00
/aws/lambda/billing   STANDARD                  14       400   15032385536   14                 0.42         0.00         0.00          0.00
/aws/lambda/checkout  STANDARD                none       365  391915765760  365                10.95        10.05         8.25          0.00
total                                                         417685569536  389                11.67        10.05         8.25          0.00
```

`--as-of` is middernacht UTC; de drie groepen zijn 10, 400 en 365 dagen vóór 2026-11-09 gemaakt. `/aws/lambda/checkout` heeft geen bewaartermijn en bevat 365 GB, dus 30 dagen bewaren zou 10,05 USD per maand besparen.

Op je eigen account geef je de export rechtstreeks door in plaats van hem eerst op te slaan:

```sh
aws logs describe-log-groups | cwlogs-retentie --as-of "$(date -u +%Y-%m-%d)" -
```

Bij meerdere pagina's (`nextToken`) sla je elke pagina op in één lege map buiten deze repository en geef je die map mee.

## Aan de slag in 5 minuten

1. Installeer met `cargo install --locked --path .` (Rust 1.85 of nieuwer).
2. Haal je export op: `aws logs describe-log-groups > /tmp/groepen.json`. Het bestand mag ook `.gz` zijn, of je geeft een map of `-` (standaardinvoer).
3. Draai `cwlogs-retentie --as-of "$(date -u +%Y-%m-%d)" /tmp/groepen.json`. Zonder `--as-of` ontbreken de besparingskolommen.
4. Lees de tabel: `estimated_usd_month` is de opslag van de bytes die al staan; `save_30_usd`, `save_90_usd` en `save_365_usd` zijn de besparing als je 30, 90 of 365 dagen bewaart en de aanvoer gelijk blijft.
5. Het advies toont `aws logs put-retention-policy ...` en voert dat nooit uit. Een kortere bewaartermijn verwijdert logs die je niet terugkrijgt; het bedrag is een schatting, geen factuur en geen voorspelling.

## Opties en gebruik

De uitvoer van `--help` (de tekst blijft Engels, net als de kolomnamen en meldingen van het programma):

```text
Estimate CloudWatch Logs storage cost from describe-log-groups JSON. Reads files only, no network.

Usage: cwlogs-retentie [OPTIONS] <FILE>...

Arguments:
  <FILE>...  `describe-log-groups` JSON files, gzip exports, directories, or `-` for standard input

Options:
      --prices <FILE>              Price file. The default is the built-in CloudWatch Logs storage list
      --as-of <YYYY-MM-DD>         Date ages are measured to, `YYYY-MM-DD`, midnight UTC. Without it, savings are omitted
      --fail-stored-bytes <BYTES>  Exit 1 when a group with no retention has storedBytes greater than this
      --json                       Print JSON instead of the text table
      --md                         Print a Markdown table instead of the text table
      --output <FILE>              Write the report to this file instead of standard output. An existing file is replaced
  -q, --quiet                      Print only the result and errors: no notes, no warnings
  -v, --verbose                    Add progress lines on standard error
  -h, --help                       Print help
  -V, --version                    Print version

Exit codes: 0 ok, 1 unretained group above --fail-stored-bytes, 2 input or usage error, 3 --quiet with --verbose or --json with --md.
```

Invoer: een bestand met één `describe-log-groups`-antwoord (`{"logGroups": [...]}`), een kale array, één loggroep-object, een gzip-bestand van een van die vormen (herkend aan de eerste twee bytes, niet aan de naam), een map met zulke bestanden, of `-` voor standaardinvoer.

Een prijsbestand (`--prices`) is een JSON-object met `currency`, `source`, `date`, `region` en de drie tarieven `standard_storage_per_gb_month`, `infrequent_access_storage_per_gb_month` en `delivery_storage_per_gb_month` als decimale strings. Het ingebouwde bestand is `prices/cloudwatch-logs-storage.json`:

```json
{
  "currency": "USD",
  "source": "https://aws.amazon.com/cloudwatch/pricing/",
  "date": "2026-11-09",
  "region": "us-east-1",
  "standard_storage_per_gb_month": "0.03",
  "infrequent_access_storage_per_gb_month": "0.03",
  "delivery_storage_per_gb_month": "0.03"
}
```

Staan de prijsgegevens (de ingebouwde of `date` in een `--prices`-bestand) meer dan 180 dagen vóór `--as-of`, of vóór vandaag als `--as-of` ontbreekt, dan komt er een regel `warning: price data dated DATE is N days old at DAY, ...` op standaardfout en blijft de afsluitcode 0. Precies 180 dagen is niet ouder. De omgevingsvariabele `CWLOGS_RETENTIE_TODAY=YYYY-MM-DD` bepaalt welke dag "vandaag" is, zodat runs en tests niet van de klok afhangen; de tests zetten haar.

`--quiet` (`-q`) toont alleen het resultaat: in tekst de schattingsregel en de tabel (en de twee drempelregels als `--fail-stored-bytes` is gezet), zonder notities, advies of waarschuwingen, en er gaat geen waarschuwing naar standaardfout; de JSON blijft compleet. `--verbose` (`-v`) voegt `progress:`-regels toe op standaardfout en laat standaarduitvoer ongemoeid. Beide samen zijn een gebruiksfout met afsluitcode 3 en er wordt niets gelezen. De afsluitcodes 0, 1 en 2 blijven zoals ze zijn.

`--md` toont dezelfde tabel als Markdown: dezelfde bedragen, zonder andere afronding, één schattingsregel, de notities, en een `|`, backslash of regeleinde in een cel wordt ge-escaped. `--md` samen met `--json` is een gebruiksfout met afsluitcode 3.

`--output FILE` schrijft het rapport naar `FILE` in plaats van naar standaarduitvoer, die leeg blijft. Het rapport gaat naar een nieuw tijdelijk bestand in dezelfde map en wordt over `FILE` heen hernoemd, dus `FILE` is het oude rapport of het hele nieuwe. Een bestaand bestand wordt vervangen. Een symlink op `FILE` wordt zelf vervangen; het doel wordt niet gevolgd en niet gewijzigd. Een ontbrekende map wordt niet aangemaakt en een map op `FILE` wordt geweigerd; elke mislukte schrijfactie is afsluitcode 2 met `cwlogs-retentie: could not write the report to FILE: ...` en er blijft geen tijdelijk bestand achter. Een run die eerder mislukt (bijvoorbeeld een onleesbare invoer) laat `FILE` zoals het was. `--fail-stored-bytes` geeft nog steeds afsluitcode 1 en schrijft het bestand wel.

Op Unix beëindigt Ctrl-C (SIGINT) de run met afsluitcode 130 en de regel `cwlogs-retentie: interrupted by SIGINT` op standaardfout. Er wordt niets naar standaarduitvoer geschreven voordat het rapport compleet is, dus een onderbroken run laat nooit een half rapport achter of een leeg rapport dat op succes lijkt. Wordt het rapport al geschreven, dan wordt het eerst afgemaakt en beëindigt een tweede Ctrl-C de run meteen.

`--json` toont dezelfde totalen, met geld als strings. De eerste sleutel is `"schema_version": 1`, een geheel getal dat alleen verandert als een sleutel verdwijnt of van betekenis verandert; sleutels erbij zijn geen wijziging. Elk JSON-document heeft hem, ook met `--quiet`, `--verbose` of `--output`. Een mislukte run toont geen JSON, alleen een melding op standaardfout, dus er is geen JSON-foutdocument. De bron-URL en de datum staan in het document (zie "Voorbeeld").

## Voorbeeld

Een test draait elk console-blok in dit bestand en vergelijkt het met deze binary. `--as-of` is middernacht UTC. De drie groepen zijn 10, 400 en 365 dagen vóór 2026-11-09 gemaakt.

```console
$ cwlogs-retentie --as-of 2026-11-09 examples/describe-log-groups.json
prices: USD, source https://aws.amazon.com/cloudwatch/pricing/, date 2026-11-09, region us-east-1
as-of: 2026-11-09
free tier not applied: pricing examples waive the first 5 GB of archived logs per month for the account, and this export is not the whole account
not priced: ingestion, Logs Insights, Live Tail, data-protection scanning, and vended delivery tiers. Only archived storage of the stored bytes is priced.
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

log_group             class              retention  age_days  stored_bytes   gb  estimated_usd_month  save_30_usd  save_90_usd  save_365_usd
/aws/ecs/worker       INFREQUENT_ACCESS       none        10   10737418240   10                 0.30         0.00         0.00          0.00
/aws/lambda/billing   STANDARD                  14       400   15032385536   14                 0.42         0.00         0.00          0.00
/aws/lambda/checkout  STANDARD                none       365  391915765760  365                10.95        10.05         8.25          0.00
total                                                         417685569536  389                11.67        10.05         8.25          0.00

advice /aws/ecs/worker: no retention; 30 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 30
advice /aws/ecs/worker: no retention; 90 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 90
advice /aws/ecs/worker: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 365
advice /aws/lambda/checkout: no retention; 30 days saves 10.05 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 30
advice /aws/lambda/checkout: no retention; 90 days saves 8.25 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 90
advice /aws/lambda/checkout: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 365
note: savings assume steady ingest and that stored bytes are the logs still kept. A shorter retention also deletes logs you cannot get back. This is not a forecast. The command is not executed.
```

`/aws/lambda/checkout` bevat 365 GB en heeft geen bewaartermijn, dus de horizon is 365 dagen. 30 dagen bewaren laat 335 GB vervallen tegen 0,03 USD, dat is 10,05 USD per maand. 365 dagen bewaren laat niets vervallen wat nog bewaard wordt. `/aws/lambda/billing` bewaart al 14 dagen en de groep is ouder, dus de horizon is 14 dagen en elk doel bespaart 0,00 USD. `/aws/ecs/worker` is 10 dagen oud, dus 30, 90 en 365 dagen liggen allemaal op of boven de horizon. Het account-id in de voorbeeld-ARN wordt niet getoond. De adviesregel toont `put-retention-policy` en zegt dat hij niet is uitgevoerd.

Hetzelfde rapport als Markdown, met `--md`:

```console
$ cwlogs-retentie --md --as-of 2026-11-09 examples/describe-log-groups.json
# cwlogs-retentie

Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

prices: USD, source https://aws.amazon.com/cloudwatch/pricing/, date 2026-11-09, region us-east-1  
as-of: 2026-11-09

free tier not applied: pricing examples waive the first 5 GB of archived logs per month for the account, and this export is not the whole account  
not priced: ingestion, Logs Insights, Live Tail, data-protection scanning, and vended delivery tiers. Only archived storage of the stored bytes is priced.

| log_group | class | retention | age_days | stored_bytes | gb | estimated_usd_month | save_30_usd | save_90_usd | save_365_usd |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| /aws/ecs/worker | INFREQUENT_ACCESS | none | 10 | 10737418240 | 10 | 0.30 | 0.00 | 0.00 | 0.00 |
| /aws/lambda/billing | STANDARD | 14 | 400 | 15032385536 | 14 | 0.42 | 0.00 | 0.00 | 0.00 |
| /aws/lambda/checkout | STANDARD | none | 365 | 391915765760 | 365 | 10.95 | 10.05 | 8.25 | 0.00 |
| total |  |  |  | 417685569536 | 389 | 11.67 | 10.05 | 8.25 | 0.00 |

- advice /aws/ecs/worker: no retention; 30 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 30
- advice /aws/ecs/worker: no retention; 90 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 90
- advice /aws/ecs/worker: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 365
- advice /aws/lambda/checkout: no retention; 30 days saves 10.05 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 30
- advice /aws/lambda/checkout: no retention; 90 days saves 8.25 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 90
- advice /aws/lambda/checkout: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 365

note: savings assume steady ingest and that stored bytes are the logs still kept. A shorter retention also deletes logs you cannot get back. This is not a forecast. The command is not executed.
```

Hetzelfde rapport als JSON, met `--json`:

```console
$ cwlogs-retentie --json --as-of 2026-11-09 examples/describe-log-groups.json
{
  "schema_version": 1,
  "version": "1.0.0",
  "currency": "USD",
  "source": "https://aws.amazon.com/cloudwatch/pricing/",
  "date": "2026-11-09",
  "region": "us-east-1",
  "as_of": "2026-11-09",
  "estimate": true,
  "estimate_note": "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)",
  "free_tier_applied": false,
  "not_priced": "not priced: ingestion, Logs Insights, Live Tail, data-protection scanning, and vended delivery tiers. Only archived storage of the stored bytes is priced.",
  "fail_stored_bytes": null,
  "threshold_exceeded": false,
  "note": "savings assume steady ingest and that stored bytes are the logs still kept. A shorter retention also deletes logs you cannot get back. This is not a forecast. The command is not executed.",
  "warnings": [],
  "log_groups": [
    {
      "log_group": "/aws/ecs/worker",
      "class": "INFREQUENT_ACCESS",
      "retention_days": null,
      "age_days": "10",
      "stored_bytes": 10737418240,
      "gb": "10",
      "estimated_usd_month": "0.30",
      "save_30_usd": "0.00",
      "save_90_usd": "0.00",
      "save_365_usd": "0.00",
      "advice": [
        "advice /aws/ecs/worker: no retention; 30 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 30",
        "advice /aws/ecs/worker: no retention; 90 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 90",
        "advice /aws/ecs/worker: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/ecs/worker' --retention-in-days 365"
      ]
    },
    {
      "log_group": "/aws/lambda/billing",
      "class": "STANDARD",
      "retention_days": 14,
      "age_days": "400",
      "stored_bytes": 15032385536,
      "gb": "14",
      "estimated_usd_month": "0.42",
      "save_30_usd": "0.00",
      "save_90_usd": "0.00",
      "save_365_usd": "0.00",
      "advice": []
    },
    {
      "log_group": "/aws/lambda/checkout",
      "class": "STANDARD",
      "retention_days": null,
      "age_days": "365",
      "stored_bytes": 391915765760,
      "gb": "365",
      "estimated_usd_month": "10.95",
      "save_30_usd": "10.05",
      "save_90_usd": "8.25",
      "save_365_usd": "0.00",
      "advice": [
        "advice /aws/lambda/checkout: no retention; 30 days saves 10.05 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 30",
        "advice /aws/lambda/checkout: no retention; 90 days saves 8.25 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 90",
        "advice /aws/lambda/checkout: no retention; 365 days saves 0.00 USD/month if ingest stays flat. Not executed: aws logs put-retention-policy --log-group-name='/aws/lambda/checkout' --retention-in-days 365"
      ]
    }
  ],
  "totals": {
    "stored_bytes": 417685569536,
    "gb": "389",
    "estimated_usd_month": "11.67",
    "save_30_usd": "10.05",
    "save_90_usd": "8.25",
    "save_365_usd": "0.00"
  }
}
```

## Hoe het telt

C1. Eén loggroep-object is één groep. De array `logGroups` is de vorm van describe-log-groups. Een kale array en een los object worden ook geaccepteerd.
C2. De maandelijkse opslag-USD is storedBytes / 1024^3 maal het tarief van de klasse, exact uitgerekend in gehele getallen (storedBytes maal de mantisse van het tarief gedeeld door 1024^3 maal een macht van tien), zonder dat een cijfer vóór het einde wordt weggelaten. Elk USD-bedrag per rij wordt één keer half weg van nul op de cent afgerond. Het totaal is de exacte som van de rijen, één keer half weg van nul op de cent afgerond bij het tonen, dus een totaal dat precies een halve cent is, rondt omhoog af. De kolom gb toont hoogstens 6 decimalen, met dezelfde afronding. 1.024 GB is 1 TB, zoals in het prijsvoorbeeld. 30 TB tegen 0,03 USD is 921,60 USD. Het totaal van een map wordt één keer uit de exacte bedragen afgerond, dus de map kan 0.08 tonen terwijl de losse bestanden 0.03 en 0.06 tonen.
C3. Standaardopslag (archief) is 0,03 USD per GB-maand.
C4. Infrequent Access-archief in het prijsvoorbeeld is ook 0,03 USD per GB-maand. Een prijsbestand kan per klasse een ander tarief zetten.
C5. Delivery-archief in het prijsvoorbeeld is 0,03 USD per GB-maand.
C6. Bron-URL, datum en regio worden getoond. De ingebouwde prijsopgave is https://aws.amazon.com/cloudwatch/pricing/ , van 2026-11-09, regio us-east-1.
C7. De gratis 5 GB per account uit de prijsvoorbeelden wordt niet afgetrokken. 6 GB gearchiveerde logs is 0,18 USD.
C8. Ingestie, Logs Insights, Live Tail en data-protection-scans worden niet geprijsd. Vended-delivery-lagen worden niet geprijsd. Alleen gearchiveerde opslag van de opgeslagen bytes wordt geprijsd.
C9. Een ontbrekende retentionInDays betekent dat de groep nooit verloopt. JSON null betekent hetzelfde.
C10. Besparingen gebruiken een model met gelijkblijvende aanvoer: de horizon is de leeftijd als er geen bewaartermijn is, of de kortste van leeftijd en bewaartermijn als die er wel is. De bewaarde bytes na T dagen zijn storedBytes * min(T, horizon) / horizon. De bespaarde USD wordt half weg van nul op de cent afgerond. Dit is geen voorspelling.
C11. De doelen zijn 30, 90 en 365 dagen. Een doel op of boven de horizon bespaart 0.
C12. `--as-of YYYY-MM-DD` is middernacht UTC. Leeftijd is (as_of - creationTime) / 86400000 dagen. creationTime is epoch-milliseconden. De age_days in de uitvoer heeft hoogstens 6 decimalen; de besparingen gebruiken de volledige waarde.
C13. Zonder `--as-of`, of zonder creationTime, wordt de opslagkost nog steeds getoond en blijven de besparingen weg.
C14. `--fail-stored-bytes N` geeft afsluitcode 1 als een groep zonder bewaartermijn meer dan N storedBytes heeft. Een groep met bewaartermijn haalt de drempel niet. Het rapport wordt nog steeds getoond. `--json` verandert de afsluitcode niet. Is een lid van een map overgeslagen (geneste symlink, geen gewoon bestand, een bestand dat geen UTF-8 is, of een tekstnotitie), dan is de run een fout met afsluitcode 2, omdat het totaal dat lid zou missen.
C15. `--json` toont geld als strings, bevat de bron en de datum, en eindigt met een newline.
C16. De adviesregel is tekst. Hij bevat `put-retention-policy` en zegt dat hij niet is uitgevoerd. Het programma voert hem niet uit.
C17. Er is geen AWS SDK en geen netwerk. nextToken wordt niet gevolgd. Het token wordt niet getoond.
C18. Dubbele loggroepnamen zijn een fout. De ruwe JSON wordt niet getoond.
C19. storedBytes moet een niet-negatief geheel getal zijn. Exponenten en negatieve getallen worden geweigerd en niet teruggegeven.
C20. Loggroepnamen zijn 1 tot 512 tekens uit het CloudWatch-naamalfabet. Een foute naam wordt niet teruggegeven.
C21. Gzip wordt herkend aan de magische bytes 1f 8b, niet aan de bestandsnaam. Bytes na het eerste gzip-lid zijn een fout.
C22. Account-id's in ARN's worden niet getoond. Een item mag zowel `arn` als `logGroupArn` hebben, zoals de API beschrijft; ze worden geaccepteerd als ze over de regio overeenkomen en geweigerd (`both arn and logGroupArn`) als ze twee regio's noemen. Elke regio in een ARN die van de prijsregio verschilt, is één waarschuwing (groepen uit één andere regio geven één waarschuwing, groepen uit twee andere regio's geven er twee).
C23. Afgeronde rijen tellen niet op tot het totaal. Er is geen extrapolatie van aanvoer per dag. De maandkost is de opgeslagen bytes die al geteld zijn.
C24. Fouten bevatten geen bestandsinhoud. Stuur-, onzichtbare en richtingveranderende tekens in paden, namen, de prijsbron en de argumenten die een gebruiksfout teruggeeft, worden getoond als `\u{...}`-escapes, een newline als `\u{a}`, zodat een fout één regel is. In een pad of argument dat geen UTF-8 is, wordt elke ongeldige byte getoond als `\xNN` en de rest op dezelfde manier ge-escaped.

## Afsluitcodes

| Code | Betekenis |
| ---: | --- |
| 0 | Het rapport is getoond (ook met waarschuwingen en met `--quiet`). |
| 1 | Een loggroep zonder bewaartermijn heeft meer opgeslagen bytes dan `--fail-stored-bytes`. Het rapport wordt nog steeds getoond of geschreven. |
| 2 | Invoer- of gebruiksfout: een onleesbaar of ongeldig bestand, een dubbele naam, een fout prijsbestand, een bedrag dat niet past (`price overflow`), een mislukte schrijfactie van het rapport, of een gesloten of volle standaarduitvoer. De melding is één regel op standaardfout, met stuurtekens ge-escaped, en bevat geen bestandsinhoud. |
| 3 | `--quiet` met `--verbose`, of `--json` met `--md`. Er wordt niets gelezen. |
| 130 | Onderbroken met Ctrl-C (SIGINT, alleen Unix). Er is niets naar standaarduitvoer geschreven. |

Een mislukte run toont geen JSON, alleen de melding op standaardfout.

## Beperkingen

De adviesregel toont `aws logs put-retention-policy` en zegt dat het commando niet is uitgevoerd (not executed). Het programma voert het niet uit. Een kortere bewaartermijn verwijdert logs die je niet terugkrijgt. Het besparingscijfer is geen voorspelling (not a forecast).

De ingebouwde tarieven zijn de us-east-1-voorbeelden van de CloudWatch-prijspagina, gelezen op 2026-11-09. Een andere regio vraagt `--prices`. De gratis laag (free tier) per account uit die voorbeelden wordt niet toegepast, omdat één export niet het hele account is.

Ingestie, Logs Insights, Live Tail, data-protection-scans en vended-delivery-lagen vallen buiten deze tool. Er is geen kolom per dag (per day). Het maandcijfer is de opslag van de bytes die al staan.

De tool heeft geen AWS SDK en opent geen netwerk (no network). Een nextToken wordt niet gevolgd, dus één antwoordpagina is niet het hele account. Het token wordt niet getoond.

Een bestand groter dan 268435456 bytes wordt geweigerd. Een geneste symlink wordt overgeslagen met een waarschuwing. Met `--json` gaan waarschuwingen naar standaardfout. Een expliciete symlink wordt gevolgd. Een map die te diep genest is, meer dan 64 niveaus, is een fout: `directory is nested too deeply`.

Is standaarduitvoer gesloten of mislukt een schrijfactie ernaar, ook voor `--help` en `--version`, dan eindigt de tool met afsluitcode 2 en `cwlogs-retentie: could not write output` op standaardfout. Bij gesloten standaarduitvoer worden een gebruiksfout of invoerfout nog steeds op standaardfout getoond (ook afsluitcode 2). Gesloten standaarduitvoer wordt alleen op Linux gedetecteerd. Standaarduitvoer op `/dev/null` geeft nog steeds 0. Een mislukte schrijfactie van een waarschuwing naar standaardfout wordt genegeerd.

### Invoerregels

- Een loggroep heeft een `logGroupName` van 1 tot 512 tekens uit het CloudWatch-naamalfabet en een `storedBytes` die een gewone reeks cijfers is, als getal of als string. In een string worden alleen spatie, tab, CR en LF rond de cijfers genegeerd; geen ander leeg teken telt.
- `retentionInDays` is een geheel getal van 1 tot 3653, als getal of als string van cijfers (`"14"` en `"0014"` zijn 14; `"+14"` en `"1.0"` zijn `bad retentionInDays`). Een getal in dat bereik dat CloudWatch niet aanbiedt, zoals 8, wordt met een waarschuwing getoond en niet veranderd.
- Arrays en objecten die 128 of meer niveaus diep genest zijn, zijn `not JSON`; 127 niveaus worden gelezen. De envelop, de array en de groep tellen als niveaus.
- Een getal als string is hoogstens 20 tekens, ook met voorloopnullen: `"018446744073709551615"` (21 tekens) is `bad storedBytes`, en een `retentionInDays`-string van 21 of meer tekens ook, hoewel `"0014"` 14 is.
- `storedBytes` is hoogstens 18446744073709551615 (een 64-bit getal zonder teken). Het totaal van alle groepen heeft dezelfde grens: twee groepen van 18446744073709551615 bytes zijn `stored bytes overflow` en afsluitcode 2, en de melding noemt geen bestand of groep.
- Een bestand met byte order mark wordt gelezen; UTF-16 is `not UTF-8`.
- Een waarde in een prijsbestand is een gewoon decimaal getal van hoogstens 40 tekens, met hoogstens 28 cijfers achter de komma en hoogstens 28 significante cijfers. Al het andere is `bad` plus de veldnaam, omdat een langer getal naar een ander tarief zou worden afgerond. Er is geen bovengrens voor een tarief: 1000000000 USD per GB wordt geaccepteerd en getoond. Een bedrag dat niet past is `price overflow`.
- In een map wordt een lid dat een tekstbestand is, geen UTF-8 is of leeg is, met een waarschuwing overgeslagen. Een lid dat geldige JSON zonder loggroepen is, zoals `42`, wordt niet overgeslagen: het is `expected log groups` en afsluitcode 2. Met `--fail-stored-bytes` is elk overgeslagen lid afsluitcode 2.
- De prijs-`source` in `--json` is de ge-escapete tekst: een bron met U+202E is `x\\u{202e}y` in het JSON-document.

### Geldprecisie

- Bedragen worden exact berekend en één keer op de cent afgerond, half weg van nul: het maandbedrag van een rij, het totaal van alle rijen, elke besparing en het totaal van de besparingen. Bytes, het tarief (als mantisse en macht van tien) en de leeftijd van een groep (in milliseconden, als deel van een dag) worden als gehele getallen van willekeurige grootte vermenigvuldigd en gedeeld, en de rijen van een totaal worden opgeteld zonder ze eerst af te ronden. Het getoonde bedrag is het exacte bedrag voor elk tarief dat de tool accepteert. Voorbeelden die eerder één cent te laag uitkwamen: drie groepen van 161730679452, 561861173778 en 162782022482 bytes tegen het ingebouwde tarief geven als totaal `24.77` (precies 24,765), en een groep van 492042190848 bytes met een `creationTime` van 94/3 dag vóór `--as-of 2026-11-09` bespaart in 30 dagen `0.59` (precies 0,585). De tests vergelijken willekeurige groepen, leeftijden en bewaartermijnen met rekenwerk in gehele getallen.
- Het `gb`-cijfer en de leeftijd worden met hoogstens 6 decimalen getoond, half weg van nul; het is geen geld en komt in geen bedrag terecht.
- Een bedrag dat het decimale type na afronding op de cent niet kan bevatten, is `price overflow` (afsluitcode 2), nooit een afgekapt bedrag. Dat is meer dan 792281625142643375935439503.35 USD met centen, of een bedrag in hele dollars van meer dan 79228162514264337593543950335 USD. Bijvoorbeeld een tarief van `1000000000000000000` USD per GB met 18446744073709551615 bytes is `price overflow`; eerder toonde dat `17179869183999999999068677425.00`, terwijl het exacte bedrag `...425.38` is.
- Kosten: een totaal wordt opgeteld als geschaalde gehele getallen, één stap per groep, en een rij kost evenveel hoeveel rijen er al waren. Kan die geschaalde som aan beide kanten van een cent vallen, dan wordt het totaal uit de exacte bedragen bepaald. Rijen met dezelfde noemer (dezelfde leeftijd en hetzelfde tarief) worden bij het lezen in één teller opgeteld, die tellers worden herleid, en ze worden opgeteld over hun kleinste gemene veelvoud. Deze exacte stap is begrensd: hoogstens 4096 verschillende noemers en een gemeenschappelijke noemer van hoogstens 2048 bits. Een totaal dat zo dicht bij een halve cent ligt en een van beide grenzen overschrijdt, geeft de fout `total is too close to a half cent to round exactly within the work limit` (afsluitcode 2); het wordt nooit op een gok afgerond. Een exacte halve cent over maximaal 4096 verschillende leeftijden, of over een willekeurig aantal groepen dat zijn leeftijden in paren of grotere verzamelingen deelt, wordt zoals gewoonlijk half weg van nul afgerond. Gemeten met de release-binary, de teksttabel en `--as-of 2026-11-09`, de kortste van twee runs en de hoogste piek. 100000, 200000 en 400000 groepen met elk een andere leeftijd, waar de cent al uit de geschaalde som zeker is: 1,71 s en 152876 KB, 3,53 s en 305268 KB, 6,85 s en 566644 KB; vanaf de run van 100000 groepen groeide de tijd met 2,06 en 4,00 en het piekgeheugen met 2,00 en 3,71. 1000, 2000 en 4000 paren groepen (2000, 4000 en 8000 groepen) waarvan de paren verschillende leeftijden hebben en waarvan het besparingstotaal een exacte halve cent is, zodat de exacte stap draait: 0,04 s en 17020 KB, 0,08 s en 30724 KB, 0,17 s en 58712 KB; vanaf de eerste run groeide de tijd met 2,0 en 4,3 en het piekgeheugen met 1,8 en 3,4. Hetzelfde soort bestand met 50000 paren (100000 groepen) heeft meer dan 4096 verschillende noemers en eindigt na 0,73 s en 94608 KB met de bovenstaande fout.

### Omvang, tijd en systeem

- Eén run las een bestand van 268434299 bytes met 1348916 kleine groepen in 10 seconden en gebruikte op de piek 2,7 GB geheugen (releasebuild, één Linux-machine). Bytes worden na elk geparsed bestand losgelaten, dus een map telt zijn bestanden niet op in het geheugen.
- Het geheugen is niet door de bestandsgrootte begrensd. Elk veld, ook een dat de tool niet gebruikt, blijft in het geheugen tijdens het lezen: een bestand met één groep en een onbekend veld met een lijst getallen kostte 0,7 seconde en 0,53 GB bij 10000059 bytes, 2,7 seconden en 2,1 GB bij 40000059 bytes en 6,9 seconden en 5,3 GB bij 100000059 bytes, ongeveer 53 keer de bestandsgrootte. Een bestand dicht bij de groottegrens kan dus meer geheugen nodig hebben dan de machine heeft. Met `--json` namen 1 miljoen groepen (69777795 bytes) 4,6 GB, tegen 0,9 GB voor de teksttabel. Houd exports bij de groepen die je nodig hebt.
- Een gzip-bestand in een gzip-bestand is `not UTF-8`, geen gzip-fout. De fout voor een `creationTime` na `--as-of` noemt de loggroep maar niet het bestand.
- Gebouwd en getest op Linux x86_64 met glibc; de releasebinary die hier is gebouwd heeft glibc 2.34 of nieuwer nodig en een oudere glibc is niet geprobeerd. Een musl-build draait het voorbeeld en de gevallen met gesloten en volle standaarduitvoer; de testsuite is niet op musl gedraaid. De code compileert voor Windows en macOS (clippy), maar de tests zijn daar niet gedraaid.

## Prijzen en verantwoordelijkheid

De ingebouwde prijzen zijn een momentopname van https://aws.amazon.com/cloudwatch/pricing/ met peildatum 2026-11-09 (de `date` in `prices/cloudwatch-logs-storage.json`). Controleer zelf de actuele prijzen. Je bent zelf verantwoordelijk voor elke beslissing op basis van de uitvoer. Met `--prices` geef je een eigen prijsbestand mee.

## Onderhoud

Meld bugs via GitHub Issues van deze repository. Na een release staat de tool in sluimermodus. Een nieuwe versie volgt alleen bij een gemelde bug, een aanvraag of een beveiligingsprobleem; beveiliging gaat altijd voor.

## Ondersteuning

Meld een bug of stel een vraag via [GitHub Issues](https://github.com/qwpp7g79y4-maker/cwlogs-retentie/issues). Geef de versie, het commando en een minimale verzonnen invoer mee, nooit een echte export. Voor beveiligingsproblemen gebruik je de besloten route in `SECURITY.md`. Wil je bijdragen, lees dan `CONTRIBUTING.md`.

## Licentie

MIT OR Apache-2.0. Zie `LICENSE-MIT` en `LICENSE-APACHE`. De licenties van de derde-partij-crates in een release-binary staan in het bestand `THIRD-PARTY-LICENSES.html` van elk release-archief.

De licenties van de externe crates in het programma staan in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).

## Contact

- E-mail: pep_ai@icloud.com
- Website: https://www.pepai.nl
- X: https://x.com/pepai_nl
- Ko-fi: https://ko-fi.com/pepai
