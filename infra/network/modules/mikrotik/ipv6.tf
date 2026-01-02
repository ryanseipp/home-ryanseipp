# =============================================================================
# IPv6 Configuration
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ipv6_address
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ipv6_neighbor_discovery
# =============================================================================

# Assign IPv6 address from delegated prefix to each IPv6-enabled VLAN
resource "routeros_ipv6_address" "vlans" {
  for_each = local.ipv6_vlans

  interface = each.value.name
  address   = "::1/${each.value.ipv6.prefix_length}"
  from_pool = "v6-pool-pd"
  advertise = true
  comment   = "${each.value.name} VLAN IPv6 from DHCPv6-PD"
}

# Router Advertisement for SLAAC on each IPv6-enabled VLAN
resource "routeros_ipv6_neighbor_discovery" "vlans" {
  for_each = local.ipv6_vlans

  interface     = each.value.name
  ra_interval   = "3m20s-10m"
  ra_lifetime   = "30m"
  advertise_dns = length(each.value.ipv6.dns_servers) > 0
  dns           = join(",", each.value.ipv6.dns_servers)
}
