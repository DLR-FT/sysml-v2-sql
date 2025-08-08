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
