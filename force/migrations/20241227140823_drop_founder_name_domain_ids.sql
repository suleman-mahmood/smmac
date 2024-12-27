alter table email drop column founder_name_data_extract_id;
alter table email drop column domain_data_extract_id;

alter table email add column founder_name text not null;
alter table email add column domain text not null;
