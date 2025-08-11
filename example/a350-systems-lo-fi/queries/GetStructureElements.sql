-- Collects all instantiations of the definitions in the selected ATA package
SELECT
  e3.declaredName AS 'Owner',
  e2.declaredName AS 'Element Name',
  e1.declaredName AS 'Type Name'
FROM
  elements AS e1
  LEFT JOIN relations r1 ON e1."@id" = r1.target_id
  LEFT JOIN elements e2 ON r1.origin_id = e2."@id"
  LEFT JOIN relations AS r2 ON e2."@id" = r2.origin_id
  LEFT JOIN elements e3 ON e3."@id" = r2.target_id
  -- Filters for definition elements in ATA package
WHERE
  e1."@type" LIKE '%Definition'
  AND e1.qualifiedName LIKE '%StructureATAs%'
  -- Collect related instances of definitions and the owner of the instance elements
  AND r1.name = 'definition'
  AND r2.name = 'owner';
