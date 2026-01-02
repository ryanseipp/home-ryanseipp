# =================================================================================================
# IP Address
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_address
# =================================================================================================
resource "routeros_ip_address" "vlans" {
  for_each = local.ipv4_vlans

  address   = "${each.value.ipv4.gateway}/${each.value.ipv4.cidr_suffix}"
  interface = each.value.name
  network   = each.value.ipv4.network
}

# ================================================================================================
# DHCP Pool Range
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_pool
# ================================================================================================
resource "routeros_ip_pool" "this" {
  for_each = local.ipv4_vlans

  name    = "${each.value.name}-dhcp-pool"
  comment = "${each.value.name} DHCP Pool"
  ranges  = each.value.ipv4.dhcp_pool
}

# ================================================================================================
# DHCP Network Configuration
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_dhcp_server_network
# ================================================================================================
resource "routeros_ip_dhcp_server_network" "this" {
  for_each = local.ipv4_vlans

  comment    = "${each.value.name} DHCP Network"
  address    = "${each.value.ipv4.network}/${each.value.ipv4.cidr_suffix}"
  gateway    = each.value.ipv4.gateway
  dns_server = each.value.ipv4.dns_servers
}

# ================================================================================================
# DHCP Server Configuration
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_dhcp_server
# ================================================================================================
resource "routeros_ip_dhcp_server" "this" {
  for_each = local.ipv4_vlans

  name               = each.value.name
  comment            = "${each.value.name} DHCP Server"
  address_pool       = routeros_ip_pool.this[each.key].name
  interface          = each.value.name
  client_mac_limit   = 1
  conflict_detection = false
}

# ================================================================================================
# Static DHCP Leases
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_dhcp_server_lease
# ================================================================================================
locals {
  static_leases = merge([
    for vlan_key, vlan in local.ipv4_vlans : {
      for ip, lease in(vlan.ipv4.static_leases != null ? vlan.ipv4.static_leases : {}) :
      "${vlan_key}-${ip}" => {
        address   = ip
        mac       = lease.mac
        name      = lease.name
        vlan_name = vlan_key
      }
    }
  ]...)
}

resource "routeros_ip_dhcp_server_lease" "this" {
  for_each = local.static_leases

  server      = routeros_ip_dhcp_server.this[each.value.vlan_name].name
  address     = each.value.address
  mac_address = each.value.mac
  comment     = each.value.name
}
