-- Select resulting properties
SELECT
  declaredName,
  "@type",
  "@id"
FROM
  elements
  -- Define property filter for elements
WHERE
  "@type" LIKE '%Part%';
