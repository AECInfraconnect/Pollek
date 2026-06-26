package pollek.policies.daily_cost
import future.keywords.if

default allow := true
max_daily_cost_usd := 25.00
allow := false if {
  input.cost.currency == "USD"
  input.cost.total_cost > max_daily_cost_usd
}
