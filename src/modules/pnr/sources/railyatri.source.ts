import { DataSource } from '../../core/AgentAggregator';

export class RailyatriPnrSource implements DataSource<any> {
  name = 'Railyatri';

  async fetch(pnr: string): Promise<any> {
    const axios = (await import('axios')).default;
    const cheerio = await import('cheerio');
    
    const response = await axios.get(`https://www.railyatri.in/pnr-status/${pnr}`, {
      headers: { 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36' }
    });
    
    const $ = cheerio.load(response.data);
    const flushedText = $('#status_not_fetched p').text() || $('.pnr-sts').text();
    if (flushedText && flushedText.includes('FLUSHED PNR')) {
       throw new Error("Flushed PNR or PNR not yet generated.");
    }
    
    const trainInfoContainer = $('.pnr-search-result-info');
    if (!trainInfoContainer.length) {
       throw new Error("Could not parse PNR data from Railyatri.");
    }

    // Extract basic train info
    const trainNameRaw = trainInfoContainer.find('.train-info a span').first().text().replace(/\\s+/g, ' ').trim();
    let trainNumber = "Unknown";
    let trainName = "Unknown";
    if (trainNameRaw) {
      const match = trainNameRaw.match(/^(\\d+)\\s*(?:‒|-)?\\s*(.+)$/);
      if (match) {
        trainNumber = match[1].trim();
        trainName = match[2].trim();
      } else {
        trainName = trainNameRaw;
      }
    }

    // Extract boarding / destination
    const routeDivs = trainInfoContainer.find('.train-route .col-xs-4');
    const fromParts = $(routeDivs[0]).find('.pnr-bold-txt').text().split('|');
    const fromTime = $(routeDivs[0]).find('p:last-child').text().trim();
    
    const toParts = $(routeDivs[1]).find('.pnr-bold-txt').text().split('|');
    const toTime = $(routeDivs[1]).find('p:last-child').text().trim();

    const boardingDetailsDivs = trainInfoContainer.find('.boarding-detls .col-xs-4');
    const journeyDate = $(boardingDetailsDivs[0]).find('.pnr-bold-txt').text().trim();
    const travelClass = $(boardingDetailsDivs[1]).find('.pnr-bold-txt').text().trim();

    // Passengers
    const passengers: any[] = [];
    $('.PNRPasList').each((_, el) => {
      const cols = $(el).find('.col-xs-4 .statusType');
      if (cols.length >= 3) {
        let bookingStatus = $(cols[0]).text().replace(/\\s+/g, ' ').trim();
        let currentStatus = $(cols[1]).text().replace(/\\s+/g, ' ').trim();
        let coachBerth = $(cols[2]).text().replace(/\\s+/g, ' ').trim();
        
        let coach = "", berth = "";
        if (coachBerth.includes('/')) {
           const cbParts = coachBerth.split('/');
           coach = cbParts[0].trim();
           berth = cbParts[1].trim();
        } else {
           coach = coachBerth;
        }

        // Standardize CNF
        if (currentStatus.toLowerCase() === 'confirmed') currentStatus = 'CNF';

        passengers.push({
          booking_status: bookingStatus,
          current_status: currentStatus,
          coach: coach,
          berth: berth
        });
      }
    });

    // Check if the overall current status in the header is CNF
    let overallStatus = "Unknown";
    $('.pnr-search-result-title .col-xs-4').each((_, el) => {
      if ($(el).text().includes('CURRENT STATUS')) {
        overallStatus = $(el).find('.pnr-bold-txt').text().trim();
      }
    });

    // Handle edge case where we couldn't parse passengers but it says CNF
    if (passengers.length === 0 && overallStatus !== "Unknown") {
       passengers.push({
         booking_status: "Unknown",
         current_status: overallStatus,
         coach: "",
         berth: ""
       });
    }

    return {
      pnr,
      train_number: trainNumber,
      train_name: trainName,
      journey_date: journeyDate,
      from: { 
        code: fromParts[1] ? fromParts[1].trim() : "UNK", 
        name: fromParts[0] ? fromParts[0].trim() : "Unknown", 
        time: fromTime || "--:--", 
        day: 1 
      },
      to: { 
        code: toParts[1] ? toParts[1].trim() : "UNK", 
        name: toParts[0] ? toParts[0].trim() : "Unknown", 
        time: toTime || "--:--", 
        day: 1 
      },
      passengers: passengers,
      last_updated: new Date().toISOString(),
      freshness: "live",
      notice: `Live data extracted from Railyatri. Class: ${travelClass}`
    };
  }
}
