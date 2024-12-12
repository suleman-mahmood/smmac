create type ElementType as enum (
	'SPAN',
	'H_THREE'
);

create type EmailVerifiedStatus as enum (
	'PENDING',
	'VERIFIED',
	'INVALID'
);

create table product (
	id uuid primary key,
	niche text not null,
	product text not null,
	domain_search_url text not null,
	created_at timestamptz not null default now()
);

create table domain (
	id uuid primary key,
	product_id uuid not null references product(id),
	domain_candidate_url text not null,
	domain text,
	founder_search_url text,
	created_at timestamptz not null default now()
);

create table founder (
	id uuid primary key,
	domain text,
	element_content text not null,
	element_type ElementType not null,
	founder_name text,
	created_at timestamptz not null default now()
);

create table email (
	id uuid primary key,
	founder_id uuid not null references founder(id),
	email_address text not null unique,
	verified_status EmailVerifiedStatus not null,
	created_at timestamptz not null default now()
);
