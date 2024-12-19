create table html_page_source (
	id uuid primary key,
	html_page_source text not null,
	page_number int not null,
	product_id uuid references product(id),
	domain_id uuid references domain(id),
	created_at timestamptz not null default now()
)
