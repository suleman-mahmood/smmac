create table smart_scout (
  id bigint primary key generated always as identity,
  public_id bigint,
  name text,
  primaryCategoryId int,
  primaryCategory text,
  primarySubCategory text,
  businessName text,
  amazonSellerId text,
  estimateSales real,
  avgPrice real,
  percentFba real,
  numberReviewsLifetime int,
  numberReviews30Days int,
  numberWinningBrands int,
  numberAsins int,
  numberTopAsins int,
  street text,
  city text,
  state text,
  country text,
  zipCode text,
  numBrands1000 int,
  moMGrowth real,
  moMGrowthCount int,
  startedSellingDate text
)
