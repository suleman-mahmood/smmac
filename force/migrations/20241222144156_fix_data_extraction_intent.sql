alter table google_webpage drop column data_extraction_intent;
drop type DataExtractionIntent;
create type DataExtractionIntent as enum (
  'DOMAIN',
  'FOUNDER_NAME',
  'COMPANY_NAME'
);
alter table google_webpage add column data_extraction_intent DataExtractionIntent not null;

alter table html_tag drop column html_tag_type;
drop type HtmlTagType;
create type HtmlTagType as enum (
  'A_TAG',
  'H3_TAG',
  'NEXT_PAGE_A_TAG'
);
alter table html_tag add column html_tag_type HtmlTagType not null;

alter table data_extract drop column data_type;
drop type DataType;
create type DataType as enum (
  'DOMAIN',
  'FOUNDER_NAME'
);
alter table data_extract add column data_type DataType not null;
