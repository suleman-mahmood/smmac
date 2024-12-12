alter table product
add constraint unique_product unique (product);

alter table product
add constraint unique_domain_search_url unique (domain_search_url);

alter table domain
add constraint unique_domain_candidate_url unique (domain_candidate_url);

alter table founder
add constraint unique_founder_name_domain unique (founder_name, domain);

alter table founder
alter column domain set not null;

alter table founder
alter column founder_name set not null;

