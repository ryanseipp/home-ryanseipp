resource "routeros_ip_dhcp_client" "wan" {
  for_each = { for k, v in var.ethernet_interfaces : k => v if v.wan_port }

  interface         = each.key
  add_default_route = "yes"
  use_peer_dns      = false
  use_peer_ntp      = false
}

resource "routeros_ipv6_dhcp_client" "wan" {
  for_each = { for k, v in var.ethernet_interfaces : k => v if v.wan_port }

  pool_name          = "v6-pool-pd"
  interface          = each.key
  add_default_route  = true
  pool_prefix_length = 64
  # Request specific prefix to improve stability with Comcast
  # Update this if ISP assigns a different /60
  prefix_hint  = "2601:540:380:3cf0::/60"
  request      = ["prefix"]
  rapid_commit = true
  disabled     = false
}
