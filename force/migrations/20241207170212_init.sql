create type ElementType as enum (
	'SPAN',
	'H_THREE'
);

create table product (
	id uuid primary key,
	niche text not null,
	product text not null,
	domain_search_url text not null, -- make unique
	created_at timestamptz not null default now()
);

create table domain (
	id uuid primary key,
	product_id uuid not null references product(id),
	domain_candidate_url text not null, -- make unique
	domain text, -- duplicates
	founder_search_url text, -- duplicates
	created_at timestamptz not null default now()
);

create table founder (
	id uuid primary key,
	domain text, -- duplicates
	element_content text not null,
	element_type ElementType not null,
	founder_name text, -- duplicates
	created_at timestamptz not null default now()
	-- make founder_name and domain unique
);

create table email (
	id uuid primary key,
	founder_id uuid not null references founder(id),
	email_address text not null unique,
	verified bool not null,
	created_at timestamptz not null default now()
);
