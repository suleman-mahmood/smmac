create table config (
  id bigint primary key generated always as identity,
  key text not null,
  value text not null,
	created_at timestamptz not null default now()
);

create table niche (
  id bigint primary key generated always as identity,
  user_niche text not null,
  gippity_prompt text not null,
  generated_product text not null,
	created_at timestamptz not null default now(),

  unique (user_niche, generated_product)
);

create type DataExtractionIntent as enum (
  'Domain',
  'FounderName',
  'CompanyName'
);

create table google_webpage (
  id bigint primary key generated always as identity,
  search_query text not null,
  page_source text not null,
  page_number int not null,
  data_extraction_intent DataExtractionIntent not null,
  any_result bool not null,
	created_at timestamptz not null default now()
);

create type HtmlTagType as enum (
  'A_Tag',
  'H3_Tag',
  'NEXT_PAGE_A_TAG'
);

create table html_tag (
  id bigint primary key generated always as identity,
  text_content text not null,
  html_tag_type HtmlTagType not null,
  google_webpage_id bigint not null references google_webpage(id),
	created_at timestamptz not null default now()
);

create type DataType as enum (
  'Domain',
  'FounderName'
);

create table data_extract (
  id bigint primary key generated always as identity,
  data text not null,
  data_type DataType not null,
  html_tag_id bigint not null references html_tag(id),
	created_at timestamptz not null default now()
);

create type VerificationStatus as enum (
  'PENDING',
  'VERIFIED',
  'INVALID',
  'CATCH_ALL'
);

alter table email rename to email_old;

create table email (
  id bigint primary key generated always as identity,
  email_address text not null,
  verification_status VerificationStatus not null,
  reachability  Reachability not null,

  founder_name_data_extract_id bigint not null references data_extract(id),
  domain_data_extract_id bigint not null references data_extract(id),

	created_at timestamptz not null default now()
);

create unique index idx_unique_email_email_address on email (email_address);
create index idx_hash_email_email_address on email using hash (email_address);
