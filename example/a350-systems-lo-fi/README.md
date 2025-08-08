# Low-Fidelity A350 Example

## Overview

This example consists of the following files:

- `AviationExample.sysml`: the actual model.
- `AviationLibraryATA.sysml`: a library with common aircraft system building blocks.
- `model.json`: The JSON representation of the model when fetched from a SysML-v2 API compliant model server.
- `model.svg`: A visual rendering of the entire model.
- `README.md`: This file.

The model contains a low-fidelity (only a handful of systems) model of an A350. This is the entire model:

![A350 Lo-Fi Example](./model.svg)

## Workflow

The following steps are:

1. Initialize a database with the respective schema.
2. Import a model into the database;
   a. from either a file, **or**
   b. from an SysML-v2 API compliant model server.
3. Query the model.

To use our the `sysml-v2-sql` tool, the following steps are required:

```sh
# Step 1.
sysml-v2-sql my-db.sqlite init-db

# Step 2.a.
sysml-v2-sql my-db.sqlite import-json ./model.json

# Step 2.b.
sysml-v2-sql my-db.sqlite fetch "https://your-sysml-v2-modelserver:8000/sysmlv2-api/api" project-name "AviationExample" default-branch

# Step 3
sqlite3 -box -echo my-db.sqlite < *.sql
```

In particular the `fetch` sub-command has many options worth checking out `sysml-v2-sql my-db.sqlite fetch help`. For example, it supports HTTPS, optionally ignoring the TLS certificate validity for HTTPS (not recommended!), HTTP basic auth and dumping the fetched data to a JSON file (just like `model.json`).

## Expected Result

Executing the workflow (skipping 2.b., as we already provide the `model.json`), should yield something akin the following:

```
-- Collects all instantiations of the definititions in the selected ATA package
SELECT
  e3.declaredname AS 'Owner',
  e2.declaredname AS 'Element Name',
  e1.declaredname AS 'Type Name'
FROM
  elements AS e1
  LEFT JOIN relations r1 ON e1."@id" = r1.target_id
  LEFT JOIN elements e2 ON r1.origin_id = e2."@id"
  LEFT JOIN relations AS r2 ON e2."@id" = r2.origin_id
  LEFT JOIN elements e3 ON e3."@id" = r2.target_id
  -- Filters for definition elements in ATA package
WHERE
  e1."@type" LIKE '%Definition'
  AND e1.qualifiedname LIKE '%AircraftSystemsATAs%'
  -- Collect related instances of definitions and the owner of the instance elements
  AND r1.name = 'definition'
  AND r2.name = 'owner';
┌───────────────────────┬───────────────────────┬─────────────────────┐
│         Owner         │     Element Name      │      Type Name      │
├───────────────────────┼───────────────────────┼─────────────────────┤
│ Systems               │ CMS                   │ CabinSystem         │
│ CMS                   │ GPU                   │ IntegratedCircuit   │
│ FlightControlComputer │ CPU                   │ IntegratedCircuit   │
│ Systems               │ Transponder           │ CommunicationSystem │
│ Systems               │ AirCondition          │ AirConditioning     │
│ Systems               │ FlightControlComputer │ FlightControl       │
└───────────────────────┴───────────────────────┴─────────────────────┘
-- Collects all instantiations of the definititions in the selected ATA package
SELECT
  e3.declaredname AS 'Owner',
  e2.declaredname AS 'Element Name',
  e1.declaredname AS 'Type Name'
FROM
  elements AS e1
  LEFT JOIN relations r1 ON e1."@id" = r1.target_id
  LEFT JOIN elements e2 ON r1.origin_id = e2."@id"
  LEFT JOIN relations AS r2 ON e2."@id" = r2.origin_id
  LEFT JOIN elements e3 ON e3."@id" = r2.target_id
  -- Filters for definition elements in ATA package
WHERE
  e1."@type" LIKE '%Definition'
  AND e1.qualifiedname LIKE '%StructureATAs%'
  -- Collect related instances of definitions and the owner of the instance elements
  AND r1.name = 'definition'
  AND r2.name = 'owner';
┌───────┬──────────────┬───────────┐
│ Owner │ Element Name │ Type Name │
├───────┼──────────────┼───────────┤
│ Body  │ RightWing    │ Wing      │
│ Body  │ LeftWing     │ Wing      │
│ Body  │ LongFusalage │ Fusalage  │
└───────┴──────────────┴───────────┘
-- Select resulting properties
SELECT
  DECLAREDNAME,
  "@type",
  "@id"
FROM
  ELEMENTS
  -- Define property filter for elements
WHERE
  "@type" LIKE '%Part%'
  AND ISLIBRARYELEMENT = 0;
┌───────────────────────┬───────────┬──────────────────────────────────────┐
│     declaredName      │   @type   │                 @id                  │
├───────────────────────┼───────────┼──────────────────────────────────────┤
│ CMS                   │ PartUsage │ 28a35dbd-f427-4e3b-990e-075ffe6eb1f5 │
│ GPU                   │ PartUsage │ 2f133482-9b99-4a30-81d9-90d2f80a8606 │
│ CPU                   │ PartUsage │ 42de2c63-8c64-46eb-be92-4798dd2ebb18 │
│ Systems               │ PartUsage │ 51e50152-3c91-4f39-8bc1-12cf8adf8045 │
│ RightWing             │ PartUsage │ 524e402d-ec5d-4797-be1b-f7a4f2e3f3d3 │
│ LeftWing              │ PartUsage │ 781a48c3-4b2b-484a-accf-47c5e08bbedb │
│ A350                  │ PartUsage │ 850d9e98-453c-4ccb-935a-f5ce70f4604c │
│ LongFusalage          │ PartUsage │ ae3c15aa-1343-4ed4-b394-9d2c61dd577e │
│ Transponder           │ PartUsage │ b2bbf067-d8dd-4928-a696-6664f2b69061 │
│ Body                  │ PartUsage │ c3b237db-8b19-496b-b873-00fc11d91dc9 │
│ AirCondition          │ PartUsage │ cb25eabd-ea33-403c-ba5b-7d96f0423232 │
│ FlightControlComputer │ PartUsage │ db19835a-4502-4699-991c-6d82f5b9f7b0 │
└───────────────────────┴───────────┴──────────────────────────────────────┘
-- Select resulting properties
SELECT
  DECLAREDNAME,
  "@type",
  "@id"
FROM
  ELEMENTS
  -- Define property filter for elements
WHERE
  "@type" LIKE '%Part%';
┌───────────────────────┬────────────────┬──────────────────────────────────────┐
│     declaredName      │     @type      │                 @id                  │
├───────────────────────┼────────────────┼──────────────────────────────────────┤
│ CMS                   │ PartUsage      │ 28a35dbd-f427-4e3b-990e-075ffe6eb1f5 │
│ GPU                   │ PartUsage      │ 2f133482-9b99-4a30-81d9-90d2f80a8606 │
│ IntegratedCircuit     │ PartDefinition │ 3c9270aa-9baf-41c5-8835-071930ba8738 │
│ CPU                   │ PartUsage      │ 42de2c63-8c64-46eb-be92-4798dd2ebb18 │
│ CommunicationSystem   │ PartDefinition │ 435c5428-399c-45ef-ae4f-26a686cbe976 │
│ CabinSystem           │ PartDefinition │ 4822de0d-637d-492e-92c8-dfd84638a5db │
│ Systems               │ PartUsage      │ 51e50152-3c91-4f39-8bc1-12cf8adf8045 │
│ RightWing             │ PartUsage      │ 524e402d-ec5d-4797-be1b-f7a4f2e3f3d3 │
│ Fusalage              │ PartDefinition │ 6fdc28da-d709-41a8-aef2-921d12146b75 │
│ Wing                  │ PartDefinition │ 72d3b964-431a-4f5e-97de-5d6db239595d │
│ LandingGear           │ PartDefinition │ 769554f7-f4f1-405b-891a-4473074b3475 │
│ LeftWing              │ PartUsage      │ 781a48c3-4b2b-484a-accf-47c5e08bbedb │
│ AirConditioning       │ PartDefinition │ 7ea9950b-b7ad-4cd0-b2ed-d10cb88dcbf0 │
│ A350                  │ PartUsage      │ 850d9e98-453c-4ccb-935a-f5ce70f4604c │
│ Door                  │ PartDefinition │ 8c6afde4-1ab6-4682-b67a-fb2db291f2df │
│ LongFusalage          │ PartUsage      │ ae3c15aa-1343-4ed4-b394-9d2c61dd577e │
│ AircraftSystem        │ PartDefinition │ afa0994b-431b-416d-b7bf-2d8d5b13d2fb │
│ Transponder           │ PartUsage      │ b2bbf067-d8dd-4928-a696-6664f2b69061 │
│ Body                  │ PartUsage      │ c3b237db-8b19-496b-b873-00fc11d91dc9 │
│ AirCondition          │ PartUsage      │ cb25eabd-ea33-403c-ba5b-7d96f0423232 │
│ Structure             │ PartDefinition │ d74bb507-4569-47d4-9d05-dc9a7d4a8184 │
│ Aircraft              │ PartDefinition │ d7f9f591-0070-4a26-9673-5ac41e227011 │
│ FlightControlComputer │ PartUsage      │ db19835a-4502-4699-991c-6d82f5b9f7b0 │
│ FlightControl         │ PartDefinition │ e2c222fc-711a-4bfa-b472-e8111fbfa196 │
└───────────────────────┴────────────────┴──────────────────────────────────────┘
```
